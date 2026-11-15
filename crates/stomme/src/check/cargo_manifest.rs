use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context as _;
use serde::de::DeserializeOwned;

use super::{Problem, Rule, describe_toml_error};

pub(super) const ROOT_MANIFEST: &str = "Cargo.toml";

pub(super) fn read_manifest<T: DeserializeOwned>(
    root: &Path,
    path: &str,
    missing: &str,
) -> anyhow::Result<Result<T, Problem>> {
    let full_path = root.join(path);
    let message = match fs::read_to_string(&full_path) {
        Ok(text) => match toml::from_str(&text) {
            Ok(manifest) => return Ok(Ok(manifest)),
            Err(error) => format!(
                "is not a valid manifest: {}",
                describe_toml_error(&error, &text)
            ),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => missing.to_string(),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", full_path.display()));
        }
    };

    Ok(Err(config_problem(path.to_string(), message)))
}

pub(super) fn config_problem(path: String, message: String) -> Problem {
    Problem {
        path,
        rule: Rule::RustConfigFile,
        message,
    }
}
