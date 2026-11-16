use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::layer::{self, Catalog, Language};

pub(crate) const CONFIG_FILE: &str = "stomme.toml";
const SCHEMA: i64 = 1;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Installation {
    NoStomme,
    Valid(Selection),
    Broken(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    pub(crate) version: String,
    pub(crate) language: Language,
    pub(crate) layers: BTreeSet<String>,
    pub(crate) roles: BTreeMap<String, String>,
    pub(crate) extra_docs: Vec<String>,
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
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Exceptions {
    #[serde(default)]
    extra_docs: Vec<String>,
}

pub(crate) fn read(root: &Path, catalog: &Catalog) -> Installation {
    let text = match fs::read_to_string(root.join(CONFIG_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Installation::NoStomme,
        Err(error) => return Installation::Broken(format!("could not read the file: {error}")),
    };

    match parse(&text, catalog).and_then(|selection| validate_exceptions(root, selection)) {
        Ok(selection) => Installation::Valid(selection),
        Err(reason) => Installation::Broken(reason),
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

    let layers = layer::select(catalog, config.language, config.layers)
        .map_err(|error| error.to_string())?;

    Ok(Selection {
        version: config.version,
        language: config.language,
        layers,
        roles: config.roles,
        extra_docs: config.exceptions.extra_docs,
    })
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

    if !path.starts_with("docs/") {
        return Err(format!(
            "has an `extra_docs` path `{path}` outside `docs/`; use a path under `docs/`"
        ));
    }

    if path == "docs/stomme" || path.starts_with("docs/stomme/") {
        return Err(format!(
            "has an `extra_docs` path `{path}` inside `docs/stomme/`; synced files are not exceptions"
        ));
    }

    let target = root.join(path);

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

    Ok(())
}

#[cfg(test)]
mod tests;
