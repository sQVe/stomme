use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;
use serde_json::Value;

use super::Problem;
use crate::packages::pins;
use crate::rule::Rule;

const PACKAGE_JSON: &str = "package.json";

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    dependencies: BTreeMap<String, Value>,
    #[serde(rename = "devDependencies", default)]
    dev_dependencies: BTreeMap<String, Value>,
}

pub(super) fn check(root: &Path) -> anyhow::Result<Vec<Problem>> {
    let path = root.join(PACKAGE_JSON);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {PACKAGE_JSON}"));
        }
    };
    let manifest: Manifest = match serde_json::from_str(&text) {
        Ok(manifest) => manifest,
        Err(error) => return Ok(vec![problem(format!("is not valid JSON: {error}"))]),
    };
    let declared = manifest
        .dependencies
        .iter()
        .chain(manifest.dev_dependencies.iter());
    let problems = declared
        .filter_map(|(package, version)| version_problem(package, version))
        .collect();

    Ok(problems)
}

fn version_problem(package: &str, version: &Value) -> Option<Problem> {
    let pinned = pins().get(package)?;
    let actual = match version {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };

    if actual == *pinned {
        return None;
    }

    Some(problem(format!(
        "uses `{package}` {actual}; the pinned version is {pinned}"
    )))
}

fn problem(message: String) -> Problem {
    Problem {
        path: PACKAGE_JSON.to_string(),
        rule: Rule::PackagePin,
        message,
        key: None,
    }
}

#[cfg(test)]
pub(crate) fn write_pinned_fixture(root: &Path) {
    let manifest = serde_json::json!({ "dependencies": pins() });
    let text = serde_json::to_string_pretty(&manifest).expect("the pins form valid JSON");

    fs::write(root.join(PACKAGE_JSON), format!("{text}\n"))
        .expect("write the fixture package.json");
}

#[cfg(test)]
mod tests;
