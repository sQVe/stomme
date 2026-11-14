use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

pub(crate) const CONFIG_FILE: &str = "stomme.toml";
const SCHEMA: i64 = 1;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Installation {
    NoStomme,
    Valid(Config),
    Broken(String),
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    kind: Kind,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    RustGpuiApp,
    RustCli,
    TsWebApp,
    TsPackage,
}

pub(crate) fn read(root: &Path) -> Installation {
    let text = match fs::read_to_string(root.join(CONFIG_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Installation::NoStomme,
        Err(error) => return Installation::Broken(format!("could not read the file: {error}")),
    };

    match parse(&text) {
        Ok(config) => Installation::Valid(config),
        Err(reason) => Installation::Broken(reason),
    }
}

fn parse(text: &str) -> Result<Config, String> {
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

    toml::Value::Table(table)
        .try_into()
        .map_err(|error: toml::de::Error| {
            format!("does not match schema {SCHEMA}: {}", error.message())
        })
}

#[cfg(test)]
mod tests;
