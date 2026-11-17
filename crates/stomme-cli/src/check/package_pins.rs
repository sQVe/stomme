use std::collections::BTreeMap;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::Path;

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

pub(super) fn check(package: Option<&Result<Value, serde_json::Error>>) -> Vec<Problem> {
    let Some(package) = package else {
        return Vec::new();
    };
    let value = match package {
        Ok(value) => value,
        Err(error) => return vec![problem(format!("is not valid JSON: {error}"))],
    };
    let manifest: Manifest = match serde_json::from_value(value.clone()) {
        Ok(manifest) => manifest,
        Err(error) => return vec![problem(format!("is not valid JSON: {error}"))],
    };
    let declared = manifest
        .dependencies
        .iter()
        .chain(manifest.dev_dependencies.iter());

    declared
        .filter_map(|(package, version)| version_problem(package, version))
        .collect()
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
