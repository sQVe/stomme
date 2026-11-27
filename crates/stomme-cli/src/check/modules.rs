mod check_file;
mod resolution;
mod scopes;
mod test_code;
mod use_paths;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use super::Problem;
use super::cargo_manifest;
use super::source_files::{self, SourceFile, Tree};
use crate::config::ModuleRow;
use crate::layer::Language;
use crate::rule::Rule;
use check_file::check_file;
use resolution::Resolver;
use test_code::test_files;

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

enum ManifestRead {
    Found(cargo_manifest::PackageManifest),
    Missing,
    Invalid,
}

pub(super) fn check(
    root: &Path,
    modules: &BTreeMap<String, ModuleRow>,
) -> anyhow::Result<Vec<Problem>> {
    if modules.is_empty() {
        return Ok(Vec::new());
    }

    let tree = source_files::walk(root, Language::Rust)?;
    let (modules, mut problems) = read_modules(root, modules)?;
    let src_folders: BTreeSet<String> = modules
        .iter()
        .map(|module| module.src_folder.clone())
        .collect();
    let sources = read_sources(&tree, &modules, &src_folders)?;
    let resolver = Resolver::new(&tree, &modules, &sources);
    let test_files = test_files(&sources, &src_folders);

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

fn read_modules(
    root: &Path,
    rows: &BTreeMap<String, ModuleRow>,
) -> anyhow::Result<(Vec<Module>, Vec<Problem>)> {
    let mut modules = Vec::new();
    let mut problems = Vec::new();
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
                let edition = read_edition(root, &src_folder, &mut problems)?;
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

    Ok((modules, problems))
}

fn read_edition(
    root: &Path,
    src_folder: &str,
    problems: &mut Vec<Problem>,
) -> anyhow::Result<Edition> {
    let package = package_folder(src_folder);
    let manifest = match read_manifest(root, package, problems)? {
        ManifestRead::Found(manifest) => manifest,
        ManifestRead::Missing | ManifestRead::Invalid => return Ok(Edition::Other),
    };
    let Some(package_section) = manifest.package.as_ref() else {
        return Ok(Edition::Other);
    };
    let Some(edition) = package_section.edition.as_ref() else {
        return Ok(Edition::Rust2015);
    };

    match edition {
        toml::Value::Table(table) if inherits_workspace(table) => {
            read_workspace_edition(root, package, &manifest, problems)
        }
        _ => Ok(edition_from_value(edition)),
    }
}

fn read_workspace_edition(
    root: &Path,
    package: &str,
    manifest: &cargo_manifest::PackageManifest,
    problems: &mut Vec<Problem>,
) -> anyhow::Result<Edition> {
    if let Some(folder) = explicit_workspace_folder(package, manifest) {
        match read_manifest(root, &folder, problems)? {
            ManifestRead::Found(workspace_manifest) => {
                if let Some(workspace) = workspace_manifest.workspace.as_ref() {
                    return Ok(workspace_edition(workspace));
                }
            }
            ManifestRead::Missing => {}
            ManifestRead::Invalid => return Ok(Edition::Other),
        }
    }

    if let Some(workspace) = manifest.workspace.as_ref() {
        return Ok(workspace_edition(workspace));
    }

    let mut folder = package;

    while let Some(parent) = parent_folder(folder) {
        folder = parent;

        let workspace = match read_manifest(root, folder, problems)? {
            ManifestRead::Found(manifest) => manifest,
            ManifestRead::Missing => continue,
            ManifestRead::Invalid => return Ok(Edition::Other),
        };

        if let Some(workspace) = workspace.workspace.as_ref() {
            return Ok(workspace_edition(workspace));
        }
    }

    Ok(Edition::Other)
}

fn explicit_workspace_folder(
    package: &str,
    manifest: &cargo_manifest::PackageManifest,
) -> Option<String> {
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

fn workspace_edition(workspace: &cargo_manifest::WorkspaceSection) -> Edition {
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

fn read_manifest(
    root: &Path,
    folder: &str,
    problems: &mut Vec<Problem>,
) -> anyhow::Result<ManifestRead> {
    match cargo_manifest::read_package_manifest(root, folder)? {
        Ok(Some(manifest)) => Ok(ManifestRead::Found(manifest)),
        Ok(None) => Ok(ManifestRead::Missing),
        Err(problem) => {
            problems.push(problem);
            Ok(ManifestRead::Invalid)
        }
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

#[cfg(test)]
mod tests;
