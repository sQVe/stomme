use std::path::Path;

use serde_json::{Map, Value};

use super::{SyncError, read_existing};
use crate::packages;

const PACKAGE_JSON: &str = "package.json";
const DEPENDENCY_FIELDS: [&str; 2] = ["dependencies", "devDependencies"];

pub(super) fn plan(root: &Path) -> Result<Option<(String, String)>, SyncError> {
    let Some(bytes) = read_existing(root, PACKAGE_JSON)? else {
        return Ok(None);
    };
    let text = String::from_utf8(bytes).map_err(|_| SyncError::NotText {
        path: PACKAGE_JSON.to_string(),
    })?;
    let mut manifest: Value =
        serde_json::from_str(&text).map_err(|error| SyncError::PackageManifest {
            reason: format!("is not valid JSON: {error}"),
        })?;
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| SyncError::PackageManifest {
            reason: "is not a JSON object".to_string(),
        })?;
    let mut changed = false;

    for field in DEPENDENCY_FIELDS {
        let Some(value) = object.get_mut(field) else {
            continue;
        };
        let dependencies = value
            .as_object_mut()
            .ok_or_else(|| SyncError::PackageManifest {
                reason: format!("`{field}` is not an object"),
            })?;

        changed |= pin_dependencies(dependencies)?;
    }

    if !changed {
        return Ok(None);
    }

    let text = serde_json::to_string_pretty(&manifest).expect("a JSON value serializes");

    Ok(Some((PACKAGE_JSON.to_string(), format!("{text}\n"))))
}

fn pin_dependencies(dependencies: &mut Map<String, Value>) -> Result<bool, SyncError> {
    let mut changed = false;

    for (package, version) in dependencies.iter_mut() {
        let Some(pinned) = packages::pins().get(package) else {
            continue;
        };
        let Value::String(actual) = version else {
            return Err(SyncError::PackageManifest {
                reason: format!("`{package}` has a version that is not a string"),
            });
        };

        if actual == pinned {
            continue;
        }

        *version = Value::String(pinned.clone());
        changed = true;
    }

    Ok(changed)
}

#[cfg(test)]
mod tests;
