use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context as _;
use serde_json::Value;

use super::Problem;
use super::cargo_lock::Lock;
use super::source_files::{self, Tree};
use crate::config::Selection;
use crate::layer::Catalog;
use crate::layer::detection::Detection;
use crate::rule::Rule;

const PACKAGE_JSON: &str = "package.json";
const LOCK_FILE: &str = "Cargo.lock";
const DEPENDENCY_FIELDS: &[&str] = &["dependencies", "devDependencies", "peerDependencies"];

pub(super) fn check(
    root: &Path,
    selection: &Selection,
    catalog: &Catalog,
    lock: Option<&Lock>,
) -> anyhow::Result<Vec<Problem>> {
    let package = read_package_json(root)?;
    let mut tree: Option<Tree> = None;
    let mut problems = Vec::new();

    for (layer, detection) in catalog.detections(selection.language) {
        if selection.layers.contains(layer) {
            continue;
        }

        let evidence = match detection {
            Detection::CargoPackage(name) => cargo_package_evidence(lock, name),
            Detection::PackageDependency(name) => {
                package_dependency_evidence(package.as_ref(), name)
            }
            Detection::PackageField(name) => package_field_evidence(package.as_ref(), name),
            Detection::FilesTogether { manifest, entry } => {
                if tree.is_none() {
                    tree = Some(source_files::walk(root, selection.language)?);
                }

                let tree = tree.as_ref().expect("the tree was just walked");

                files_together_evidence(root, tree, manifest, entry)
            }
        };

        if let Some((path, evidence)) = evidence {
            problems.push(problem(layer, path, evidence));
        }
    }

    Ok(problems)
}

fn cargo_package_evidence(lock: Option<&Lock>, name: &str) -> Option<(String, String)> {
    let lock = lock?;

    lock.has_package(name)
        .then(|| (LOCK_FILE.to_string(), format!("lists package `{name}`")))
}

fn package_dependency_evidence(package: Option<&Value>, name: &str) -> Option<(String, String)> {
    let package = package?;
    let declared = DEPENDENCY_FIELDS
        .iter()
        .filter_map(|field| package.get(field))
        .any(|dependencies| dependencies.get(name).is_some());

    declared.then(|| (PACKAGE_JSON.to_string(), format!("depends on `{name}`")))
}

fn package_field_evidence(package: Option<&Value>, name: &str) -> Option<(String, String)> {
    let package = package?;

    package
        .get(name)
        .is_some()
        .then(|| (PACKAGE_JSON.to_string(), format!("has a `{name}` field")))
}

fn files_together_evidence(
    root: &Path,
    tree: &Tree,
    manifest: &str,
    entry: &str,
) -> Option<(String, String)> {
    let path = tree
        .files
        .iter()
        .filter(|file| file.name == entry)
        .filter_map(|file| {
            let manifest_path = source_files::join(&file.folder, manifest);

            root.join(&manifest_path).is_file().then_some(manifest_path)
        })
        .min()?;

    Some((path, format!("is a mod manifest beside `{entry}`")))
}

fn read_package_json(root: &Path) -> anyhow::Result<Option<Value>> {
    let path = root.join(PACKAGE_JSON);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {PACKAGE_JSON}"));
        }
    };

    Ok(serde_json::from_str(&text).ok())
}

fn problem(layer: &str, path: String, evidence: String) -> Problem {
    Problem {
        path,
        rule: Rule::LayerDeclared,
        message: format!(
            "{evidence}, so the project uses {layer}; add `{layer}` to `layers` in stomme.toml"
        ),
        key: Some(layer.to_string()),
    }
}

#[cfg(test)]
mod tests;
