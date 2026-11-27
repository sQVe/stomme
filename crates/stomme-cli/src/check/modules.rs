mod resolution;
mod scopes;
mod test_code;
mod use_paths;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::Deserialize;

use super::Problem;
use super::cargo_manifest;
use super::source_files::{self, SourceFile, Tree};
use crate::config::ModuleRow;
use crate::layer::Language;
use crate::rule::Rule;
use resolution::Resolver;
use test_code::test_files;
use use_paths::{Namespace, UsePath, paths};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edition {
    Rust2015,
    Other,
}

impl Edition {
    pub(super) fn is_2015(self) -> bool {
        matches!(self, Self::Rust2015)
    }
}

struct Module {
    name: String,
    entry: String,
    path: Vec<String>,
    src_folder: String,
    owner_folder: String,
    public: BTreeSet<String>,
    imports: BTreeSet<String>,
    edition: Edition,
}

#[derive(Deserialize)]
struct PackageManifest {
    package: Option<PackageSection>,
    workspace: Option<WorkspaceSection>,
}

#[derive(Deserialize)]
struct PackageSection {
    edition: Option<toml::Value>,
    workspace: Option<String>,
}

#[derive(Deserialize)]
struct WorkspaceSection {
    package: Option<WorkspacePackage>,
}

#[derive(Deserialize)]
struct WorkspacePackage {
    edition: Option<toml::Value>,
}

pub(super) fn check(
    root: &Path,
    modules: &BTreeMap<String, ModuleRow>,
) -> anyhow::Result<Vec<Problem>> {
    if modules.is_empty() {
        return Ok(Vec::new());
    }

    let tree = source_files::walk(root, Language::Rust)?;
    let modules = read_modules(root, modules)?;
    let src_folders: BTreeSet<String> = modules
        .iter()
        .map(|module| module.src_folder.clone())
        .collect();
    let sources = read_sources(&tree, &modules, &src_folders)?;
    let resolver = Resolver::new(&tree, &modules, &sources);
    let test_files = test_files(&sources, &src_folders);
    let mut problems = Vec::new();

    for (file, source) in &sources {
        let owner = owner_module(&modules, &file.path);

        let Some(owner) = owner else {
            if let Some(src_folder) = src_folder_for_file(&src_folders, &file.path)
                && !is_crate_root(&file.path, &src_folder)
                && !is_under_bin(&file.path, &src_folder)
            {
                problems.push(module_listed(file));
            }

            continue;
        };

        if is_crate_root(&file.path, &owner.src_folder)
            || is_under_bin(&file.path, &owner.src_folder)
        {
            continue;
        }

        let file_is_test = test_files.contains(&file.path);
        problems.extend(check_file(&resolver, owner, file, source, file_is_test));
    }

    Ok(problems)
}

fn read_sources<'tree>(
    tree: &'tree Tree,
    modules: &[Module],
    src_folders: &BTreeSet<String>,
) -> anyhow::Result<Vec<(&'tree SourceFile, String)>> {
    let mut sources = Vec::new();

    for file in &tree.files {
        let Some(src_folder) = src_folder_for_file(src_folders, &file.path) else {
            continue;
        };
        let keeps_exports =
            owner_module(modules, &file.path).is_some_and(|owner| keeps_exports(owner, &file.path));

        if is_under_bin(&file.path, &src_folder) && !keeps_exports {
            continue;
        }

        sources.push((file, tree.read(file)?));
    }

    Ok(sources)
}

fn read_modules(root: &Path, rows: &BTreeMap<String, ModuleRow>) -> anyhow::Result<Vec<Module>> {
    let mut modules = Vec::new();
    let mut editions = BTreeMap::new();

    for (name, row) in rows {
        let src_folder = src_folder(&row.entry).expect("config checked the `src` part");
        let path = module_path(&row.entry, &src_folder);
        let owner_folder = row
            .entry
            .strip_suffix(".rs")
            .unwrap_or(&row.entry)
            .to_string();
        let edition = match editions.get(&src_folder) {
            Some(edition) => *edition,
            None => {
                let edition = read_edition(root, &src_folder)?;
                editions.insert(src_folder.clone(), edition);
                edition
            }
        };

        modules.push(Module {
            name: name.clone(),
            entry: row.entry.clone(),
            path,
            src_folder,
            owner_folder,
            public: row.public.iter().cloned().collect(),
            imports: row.imports.iter().cloned().collect(),
            edition,
        });
    }

    Ok(modules)
}

fn read_edition(root: &Path, src_folder: &str) -> anyhow::Result<Edition> {
    let package = package_folder(src_folder);
    let Some(manifest) = read_package_manifest(root, package)? else {
        return Ok(Edition::Other);
    };
    let Some(edition) = manifest
        .package
        .as_ref()
        .and_then(|package| package.edition.as_ref())
    else {
        return Ok(Edition::Other);
    };

    match edition {
        toml::Value::String(edition) if edition == "2015" => Ok(Edition::Rust2015),
        toml::Value::Table(table) if inherits_workspace(table) => {
            read_workspace_edition(root, package, &manifest)
        }
        _ => Ok(Edition::Other),
    }
}

fn read_workspace_edition(
    root: &Path,
    package: &str,
    manifest: &PackageManifest,
) -> anyhow::Result<Edition> {
    if let Some(folder) = explicit_workspace_folder(package, manifest)
        && let Some(workspace_manifest) = read_package_manifest(root, &folder)?
        && let Some(workspace) = workspace_manifest.workspace.as_ref()
        && let Some(edition) = workspace
            .package
            .as_ref()
            .and_then(|package| package.edition.as_ref())
    {
        return Ok(edition_from_value(edition));
    }

    if let Some(workspace) = manifest.workspace.as_ref() {
        return Ok(workspace_edition(workspace));
    }

    let mut folder = package;

    while let Some(parent) = parent_folder(folder) {
        folder = parent;

        let Some(manifest) = read_package_manifest(root, folder)? else {
            continue;
        };

        if let Some(workspace) = manifest.workspace.as_ref() {
            return Ok(workspace_edition(workspace));
        }
    }

    Ok(Edition::Other)
}

fn explicit_workspace_folder(package: &str, manifest: &PackageManifest) -> Option<String> {
    let workspace = manifest.package.as_ref()?.workspace.as_deref()?;

    workspace_folder(package, workspace)
}

fn workspace_folder(package: &str, workspace: &str) -> Option<String> {
    let workspace = Path::new(workspace);

    if workspace.is_absolute() {
        return None;
    }

    let mut folder = Path::new(package).to_path_buf();

    for component in workspace.components() {
        match component {
            Component::Normal(part) => folder.push(part),
            Component::ParentDir => {
                if !folder.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            _ => return None,
        }
    }

    Some(folder.to_string_lossy().into_owned())
}

fn workspace_edition(workspace: &WorkspaceSection) -> Edition {
    workspace
        .package
        .as_ref()
        .and_then(|package| package.edition.as_ref())
        .map_or(Edition::Other, edition_from_value)
}

fn edition_from_value(value: &toml::Value) -> Edition {
    match value {
        toml::Value::String(edition) if edition == "2015" => Edition::Rust2015,
        _ => Edition::Other,
    }
}

fn inherits_workspace(table: &toml::Table) -> bool {
    table
        .get("workspace")
        .and_then(toml::Value::as_bool)
        .is_some_and(|workspace| workspace)
}

fn read_package_manifest(root: &Path, folder: &str) -> anyhow::Result<Option<PackageManifest>> {
    let path = if folder.is_empty() {
        cargo_manifest::ROOT_MANIFEST.to_string()
    } else {
        format!("{folder}/{}", cargo_manifest::ROOT_MANIFEST)
    };

    match cargo_manifest::read_toml_file::<PackageManifest>(root, &path)? {
        Some(Ok(manifest)) => Ok(Some(manifest)),
        Some(Err(_)) | None => Ok(None),
    }
}

fn package_folder(src_folder: &str) -> &str {
    src_folder.strip_suffix("/src").unwrap_or_default()
}

fn parent_folder(folder: &str) -> Option<&str> {
    if folder.is_empty() {
        return None;
    }

    Some(folder.rsplit_once('/').map_or("", |(parent, _)| parent))
}

fn src_folder(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    let index = parts.iter().rposition(|part| *part == "src")?;

    Some(parts[..=index].join("/"))
}

fn module_path(path: &str, src_folder: &str) -> Vec<String> {
    let relative = path.strip_prefix(&format!("{src_folder}/")).unwrap_or(path);
    let without_extension = relative.strip_suffix(".rs").unwrap_or(relative);

    without_extension.split('/').map(str::to_string).collect()
}

fn src_folder_for_file(src_folders: &BTreeSet<String>, path: &str) -> Option<String> {
    src_folders
        .iter()
        .filter(|folder| path.starts_with(&format!("{folder}/")))
        .max_by_key(|folder| folder.len())
        .cloned()
}

fn is_crate_root(path: &str, src_folder: &str) -> bool {
    path == format!("{src_folder}/lib.rs") || path == format!("{src_folder}/main.rs")
}

fn is_under_bin(path: &str, src_folder: &str) -> bool {
    path.starts_with(&format!("{src_folder}/bin/"))
}

fn owner_module<'a>(modules: &'a [Module], path: &str) -> Option<&'a Module> {
    modules
        .iter()
        .filter(|module| owns(module, path))
        .max_by_key(|module| module.path.len())
}

fn keeps_exports(module: &Module, path: &str) -> bool {
    path == module.entry || module.public.contains(path)
}

fn owns(module: &Module, path: &str) -> bool {
    let folder_prefix = format!("{}/", module.owner_folder);

    path == module.entry || path.starts_with(&folder_prefix)
}

fn module_listed(file: &SourceFile) -> Problem {
    Problem {
        path: file.path.clone(),
        rule: Rule::ModuleListed,
        message: "belongs to no module in `[modules]`; add a row or move the file".to_string(),
        key: None,
    }
}

fn check_file(
    resolver: &Resolver,
    module: &Module,
    file: &SourceFile,
    source: &str,
    file_is_test: bool,
) -> Vec<Problem> {
    let file_path = module_path(&file.path, &module.src_folder);
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();

    for use_path in paths(source, module.edition) {
        let mut current = file_path.clone();
        current.extend(use_path.inline_modules.iter().cloned());
        let in_test_module = file_is_test || use_path.in_test_module;
        let Some(resolved) = resolve(&use_path, &current) else {
            continue;
        };
        let found = classify(resolver, module, file, &resolved, &use_path, in_test_module);

        for problem in found {
            let key = (problem.rule.id(), problem.key.clone());

            if seen.insert(key) {
                problems.push(problem);
            }
        }
    }

    problems
}

fn resolve(use_path: &UsePath, file_path: &[String]) -> Option<Vec<String>> {
    let mut current = file_path.to_vec();
    let first = use_path.segments.first().map(String::as_str);

    if first == Some("super") {
        let supers = use_path
            .segments
            .iter()
            .take_while(|segment| *segment == "super")
            .count();

        if supers > current.len() {
            return None;
        }

        current.truncate(current.len() - supers);
        current.extend(use_path.segments[supers..].iter().cloned());
    } else if first == Some("self") {
        current.extend(use_path.segments[1..].iter().cloned());
    } else {
        current.clear();
        current.extend(use_path.segments[1..].iter().cloned());
    }

    Some(current)
}

fn classify(
    resolver: &Resolver,
    module: &Module,
    file: &SourceFile,
    resolved: &[String],
    use_path: &UsePath,
    in_test_module: bool,
) -> Vec<Problem> {
    let resolved = value_path_parent(resolver, module, resolved, use_path)
        .unwrap_or_else(|| resolved.to_vec());
    let Some(found) = longest_prefix(resolver.modules, module, &resolved) else {
        let unlisted = classify_unlisted(resolver.tree, module, file, &resolved, use_path);

        return unlisted.into_iter().collect();
    };

    if found.name == module.name {
        return Vec::new();
    }

    if !module.imports.contains(&found.name) {
        let message = format!(
            "line {}: module `{}` uses module `{}`, which `[modules.{}] imports` does not list",
            use_path.line, module.name, found.name, module.name
        );
        let key = format!("{}:{}", module.name, found.name);

        return vec![problem(file, Rule::ModuleImports, message, key)];
    }

    if in_test_module {
        return Vec::new();
    }

    private_file_problems(resolver, module, found, file, &resolved, use_path)
}

fn value_path_parent(
    resolver: &Resolver,
    module: &Module,
    resolved: &[String],
    use_path: &UsePath,
) -> Option<Vec<String>> {
    if use_path.namespace != Namespace::Value {
        return None;
    }

    let is_row = resolver
        .modules
        .iter()
        .any(|row| row.src_folder == module.src_folder && row.path.as_slice() == resolved);

    if !is_row {
        return None;
    }

    let (last, parent_path) = resolved.split_last()?;
    let parent = longest_prefix(resolver.modules, module, parent_path)?;

    if !resolver.declares_value_item(parent, parent_path, last) {
        return None;
    }

    Some(parent_path.to_vec())
}

fn private_file_problems(
    resolver: &Resolver,
    module: &Module,
    found: &Module,
    file: &SourceFile,
    resolved: &[String],
    use_path: &UsePath,
) -> Vec<Problem> {
    let reached = resolver.reached_files(found, resolved, use_path.namespace);

    reached
        .into_iter()
        .filter(|used_file| !found.public.contains(used_file))
        .map(|used_file| {
            let message = format!(
                "line {}: module `{}` uses `{}`, which `[modules.{}] public` does not list",
                use_path.line, module.name, used_file, found.name
            );
            let key = format!("{}:{}", module.name, used_file);

            problem(file, Rule::ModulePrivateFile, message, key)
        })
        .collect()
}

fn classify_unlisted(
    tree: &Tree,
    module: &Module,
    file: &SourceFile,
    resolved: &[String],
    use_path: &UsePath,
) -> Option<Problem> {
    let Some(first) = resolved.first() else {
        return Some(crate_root_itself(file, module, use_path));
    };

    if first == "*" {
        return Some(crate_root_path(file, module, use_path, "*"));
    }

    let names_file = tree
        .files
        .iter()
        .any(|file| file.path == format!("{}/{first}.rs", module.src_folder));
    let names_folder = tree
        .folders
        .contains(&format!("{}/{first}", module.src_folder));

    if !names_file && !names_folder {
        return Some(crate_root_path(file, module, use_path, first));
    }

    let message = format!(
        "line {}: module `{}` uses `{first}`, which no `[modules]` row covers",
        use_path.line, module.name
    );
    let key = format!("{}:{first}", module.name);

    Some(problem(file, Rule::ModuleImports, message, key))
}

fn crate_root_itself(file: &SourceFile, module: &Module, use_path: &UsePath) -> Problem {
    let message = format!(
        "line {}: module `{}` uses the crate root itself; use a module path",
        use_path.line, module.name
    );
    let key = format!("{}:crate", module.name);

    problem(file, Rule::CrateRootPath, message, key)
}

fn crate_root_path(file: &SourceFile, module: &Module, use_path: &UsePath, name: &str) -> Problem {
    let message = format!(
        "line {}: module `{}` uses the crate root path `{name}`; use a module path",
        use_path.line, module.name
    );
    let key = format!("{}:{name}", module.name);

    problem(file, Rule::CrateRootPath, message, key)
}

fn longest_prefix<'a>(
    modules: &'a [Module],
    current: &Module,
    resolved: &[String],
) -> Option<&'a Module> {
    modules
        .iter()
        .filter(|module| module.src_folder == current.src_folder)
        .filter(|module| resolved.starts_with(&module.path))
        .max_by_key(|module| module.path.len())
}

fn problem(file: &SourceFile, rule: Rule, message: String, key: String) -> Problem {
    Problem {
        path: file.path.clone(),
        rule,
        message,
        key: Some(key),
    }
}

#[cfg(test)]
mod tests;
