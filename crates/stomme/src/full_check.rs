use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;
use thiserror::Error;

pub(crate) const TAU_CONFIG: &str = ".pi/tau.json";

#[derive(Debug, Error)]
pub(crate) enum FullCheckError {
    #[error("could not read {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error("is not valid JSON: {reason}")]
    InvalidJson { reason: String },
    #[error("needs `fullCheck.argv` as a non-empty array of strings")]
    Missing,
}

pub(crate) fn read(root: &Path) -> Result<Option<Vec<String>>, FullCheckError> {
    let path = root.join(TAU_CONFIG);

    if !path.is_file() {
        return Ok(None);
    }

    let text = fs::read_to_string(&path).map_err(|source| FullCheckError::Read {
        path: TAU_CONFIG.to_string(),
        source,
    })?;
    let config: Value =
        serde_json::from_str(&text).map_err(|error| FullCheckError::InvalidJson {
            reason: error.to_string(),
        })?;
    let Some(argv) = config.pointer("/fullCheck/argv").and_then(Value::as_array) else {
        return Err(FullCheckError::Missing);
    };

    if argv.is_empty() || !argv.iter().all(Value::is_string) {
        return Err(FullCheckError::Missing);
    }

    let argv = argv
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("every argument is a string")
                .to_string()
        })
        .collect();

    Ok(Some(argv))
}
