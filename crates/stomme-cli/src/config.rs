use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::layer::{self, Catalog, Language};
use crate::rule::Rule;

pub(crate) const CONFIG_FILE: &str = "stomme.toml";
pub(crate) const SCHEMA: i64 = 1;

#[derive(Debug)]
pub(crate) enum Installation {
    NoStomme,
    Valid(Selection),
    Broken(Reason),
}

#[derive(Debug)]
pub(crate) enum Reason {
    SymbolicLink,
    NotRegularFile,
    Unreadable { source: io::Error },
    Invalid(String),
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SymbolicLink => formatter.write_str("is a symbolic link; use a regular file"),
            Self::NotRegularFile => formatter.write_str("is not a regular file"),
            Self::Unreadable { source } => write!(formatter, "could not read the file: {source}"),
            Self::Invalid(reason) => formatter.write_str(reason),
        }
    }
}

pub(crate) struct FileRead {
    pub(crate) inspect: fn(&Path) -> io::Result<fs::Metadata>,
    pub(crate) read: fn(&Path) -> io::Result<String>,
}

impl FileRead {
    pub(crate) fn filesystem() -> Self {
        Self {
            inspect: inspect_entry,
            read: read_text,
        }
    }
}

fn inspect_entry(path: &Path) -> io::Result<fs::Metadata> {
    fs::symlink_metadata(path)
}

fn read_text(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Violation {
    pub(crate) path: String,
    pub(crate) rule: Rule,
    pub(crate) key: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    pub(crate) version: String,
    pub(crate) language: Language,
    pub(crate) layers: BTreeSet<String>,
    pub(crate) roles: BTreeMap<String, String>,
    pub(crate) violations: Vec<Violation>,
    pub(crate) extra_docs: Vec<String>,
    pub(crate) modules: BTreeMap<String, ModuleRow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    version: String,
    language: Language,
    #[serde(default)]
    layers: Vec<String>,
    #[serde(default)]
    roles: BTreeMap<String, String>,
    #[serde(default)]
    exceptions: Exceptions,
    #[serde(default)]
    modules: BTreeMap<String, ModuleRow>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Exceptions {
    #[serde(default)]
    extra_docs: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModuleRow {
    pub(crate) entry: String,
    #[serde(default)]
    pub(crate) public: Vec<String>,
    #[serde(default)]
    pub(crate) imports: Vec<String>,
}

pub(crate) fn read(root: &Path, catalog: &Catalog, file: &FileRead) -> Installation {
    let path = root.join(CONFIG_FILE);

    let text = match read_entry(&path, file) {
        Entry::Missing => return Installation::NoStomme,
        Entry::Broken(reason) => return Installation::Broken(reason),
        Entry::Text(text) => text,
    };

    match parse(&text, catalog)
        .and_then(|selection| validate_rust_modules(root, selection))
        .and_then(|selection| validate_exceptions(root, selection))
    {
        Ok(selection) => Installation::Valid(selection),
        Err(reason) => Installation::Broken(Reason::Invalid(reason)),
    }
}

enum Entry {
    Missing,
    Broken(Reason),
    Text(String),
}

fn read_entry(path: &Path, file: &FileRead) -> Entry {
    let metadata = match (file.inspect)(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Entry::Missing,
        Err(source) => return Entry::Broken(Reason::Unreadable { source }),
    };
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        return Entry::Broken(Reason::SymbolicLink);
    }

    if !file_type.is_file() {
        return Entry::Broken(Reason::NotRegularFile);
    }

    match (file.read)(path) {
        Ok(text) => Entry::Text(text),
        Err(source) => Entry::Broken(Reason::Unreadable { source }),
    }
}

fn parse(text: &str, catalog: &Catalog) -> Result<Selection, String> {
    let mut table: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| format!("is not valid TOML: {}", error.message()))?;
    let schema = table
        .remove("schema")
        .ok_or_else(|| format!("is missing `schema`; use `schema = {SCHEMA}`"))?;

    if schema.as_integer() != Some(SCHEMA) {
        return Err(format!(
            "has `schema = {schema}`; this stomme reads `schema = {SCHEMA}`"
        ));
    }

    if table.contains_key("kind") {
        return Err(format!(
            "has `kind`, which schema {SCHEMA} does not read; declare `language`, `layers`, and `[roles]` instead"
        ));
    }

    let violations = parse_violations(table.remove("violations"))?;
    let config: Config =
        toml::Value::Table(table)
            .try_into()
            .map_err(|error: toml::de::Error| {
                format!("does not match schema {SCHEMA}: {}", error.message())
            })?;

    if config.version != env!("CARGO_PKG_VERSION") {
        return Err(format!(
            "has `version = \"{}\"`; this stomme is version {}",
            config.version,
            env!("CARGO_PKG_VERSION")
        ));
    }

    validate_modules(&config.modules)?;

    let layers = layer::select(catalog, config.language, config.layers)
        .map_err(|error| error.to_string())?;

    Ok(Selection {
        version: config.version,
        language: config.language,
        layers,
        roles: config.roles,
        violations,
        extra_docs: config.exceptions.extra_docs,
        modules: config.modules,
    })
}

fn parse_violations(value: Option<toml::Value>) -> Result<Vec<Violation>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let toml::Value::Array(entries) = value else {
        return Err("has `violations` that is not a list of tables".to_string());
    };
    let mut violations = Vec::new();

    for (index, entry) in entries.into_iter().enumerate() {
        let violation = parse_violation(index, entry)?;

        if violations.contains(&violation) {
            return Err(format!(
                "has the `violations` entry for `{}` with rule `{}` and key `{}` twice",
                violation.path,
                violation.rule.id(),
                violation.key
            ));
        }

        violations.push(violation);
    }

    Ok(violations)
}

fn parse_violation(index: usize, value: toml::Value) -> Result<Violation, String> {
    let toml::Value::Table(mut table) = value else {
        return Err(format!(
            "has a `violations` entry {} that is not a table",
            index + 1
        ));
    };
    let mut label = format!("`violations` entry {}", index + 1);
    let path = violation_field(&mut table, "path", &label)?;

    label = format!("`violations` entry for `{path}`");
    let rule_id = violation_field(&mut table, "rule", &label)?;

    label = format!("`violations` entry for `{path}` with rule `{rule_id}`");
    let key = violation_field(&mut table, "key", &label)?;
    let rule = Rule::ALL
        .iter()
        .copied()
        .find(|rule| rule.id() == rule_id)
        .ok_or_else(|| {
            format!("has a {label} with key `{key}` whose rule `{rule_id}` is unknown")
        })?;

    if let Some(field) = table.keys().next() {
        return Err(format!(
            "has a {label} with key `{key}` and the unknown field `{field}`"
        ));
    }

    Ok(Violation { path, rule, key })
}

fn violation_field(table: &mut toml::Table, field: &str, label: &str) -> Result<String, String> {
    let Some(value) = table.remove(field) else {
        return Err(format!("has a {label} without `{field}`"));
    };

    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("has a {label} whose `{field}` is not a string"))
}

fn validate_modules(modules: &BTreeMap<String, ModuleRow>) -> Result<(), String> {
    for (name, row) in modules {
        for path in std::iter::once(&row.entry).chain(&row.public) {
            if Path::new(path).is_absolute() {
                return Err(format!(
                    "has a module `{name}` path `{path}` that is absolute; use a path relative to the project root"
                ));
            }

            if path.split('/').any(|part| part == "..") {
                return Err(format!(
                    "has a module `{name}` path `{path}` with a `..` part; use a path inside the project"
                ));
            }
        }

        for target in &row.imports {
            if !modules.contains_key(target) {
                return Err(format!(
                    "has a module `{name}` that imports `{target}`, which has no `[modules.{target}]` row"
                ));
            }
        }
    }

    Ok(())
}

fn validate_rust_modules(root: &Path, selection: Selection) -> Result<Selection, String> {
    if selection.language != Language::Rust {
        return Ok(selection);
    }

    let mut entries: BTreeMap<&str, &str> = BTreeMap::new();

    for (name, row) in &selection.modules {
        for path in std::iter::once(&row.entry).chain(&row.public) {
            if !path.ends_with(".rs") {
                return Err(format!(
                    "has a module `{name}` path `{path}` that does not end in `.rs`; use a Rust file"
                ));
            }

            if !path.split('/').any(|part| part == "src") {
                return Err(format!(
                    "has a module `{name}` path `{path}` with no `src` folder part; put the file under `src`"
                ));
            }

            if !root.join(path).is_file() {
                return Err(format!(
                    "has a module `{name}` path `{path}` that is not an existing file"
                ));
            }
        }

        if let Some(other) = entries.insert(&row.entry, name) {
            return Err(format!(
                "has two modules, `{other}` and `{name}`, with the entry `{}`; each module needs its own entry file",
                row.entry
            ));
        }
    }

    Ok(selection)
}

fn validate_exceptions(root: &Path, selection: Selection) -> Result<Selection, String> {
    for path in &selection.extra_docs {
        validate_extra_doc(root, path)?;
    }

    Ok(selection)
}

fn validate_extra_doc(root: &Path, path: &str) -> Result<(), String> {
    if Path::new(path).is_absolute() {
        return Err(format!(
            "has an `extra_docs` path `{path}` that is absolute; use a path under `docs/`"
        ));
    }

    if path.split('/').any(|part| part == "..") {
        return Err(format!(
            "has an `extra_docs` path `{path}` with a `..` part; use a path under `docs/`"
        ));
    }

    let normalized = normalize(path);

    if !normalized.starts_with("docs/") {
        return Err(format!(
            "has an `extra_docs` path `{path}` outside `docs/`; use a path under `docs/`"
        ));
    }

    if normalized == "docs/stomme" || normalized.starts_with("docs/stomme/") {
        return Err(format!(
            "has an `extra_docs` path `{path}` inside `docs/stomme/`; synced files are not exceptions"
        ));
    }

    let target = root.join(&normalized);

    if !target.is_file() {
        return Err(format!(
            "has an `extra_docs` path `{path}` that is not an existing file"
        ));
    }

    let docs = fs::canonicalize(root.join("docs"))
        .map_err(|error| format!("could not resolve `docs/`: {error}"))?;
    let resolved = fs::canonicalize(&target)
        .map_err(|error| format!("could not resolve `{path}`: {error}"))?;

    if !resolved.starts_with(&docs) {
        return Err(format!(
            "has an `extra_docs` path `{path}` that resolves outside `docs/`"
        ));
    }

    if let Ok(synced) = fs::canonicalize(root.join("docs/stomme"))
        && resolved.starts_with(&synced)
    {
        return Err(format!(
            "has an `extra_docs` path `{path}` that resolves inside `docs/stomme/`; synced files are not exceptions"
        ));
    }

    Ok(())
}

fn normalize(path: &str) -> String {
    path.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests;
