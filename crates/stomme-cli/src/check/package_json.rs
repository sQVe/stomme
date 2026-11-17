use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context as _;
use serde_json::Value;

const PACKAGE_JSON: &str = "package.json";

pub(super) fn read(root: &Path) -> anyhow::Result<Option<Result<Value, serde_json::Error>>> {
    let path = root.join(PACKAGE_JSON);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {PACKAGE_JSON}"));
        }
    };

    Ok(Some(serde_json::from_str(&text)))
}
