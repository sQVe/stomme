use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::layer::{self, Language};

pub(crate) const CONFIG_FILE: &str = "stomme.toml";
const SCHEMA: i64 = 1;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Installation {
    NoStomme,
    Valid(Selection),
    Broken(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    pub(crate) language: Language,
    pub(crate) layers: BTreeSet<String>,
    pub(crate) roles: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    language: Language,
    #[serde(default)]
    layers: Vec<String>,
    #[serde(default)]
    roles: BTreeMap<String, String>,
}

pub(crate) fn read(root: &Path) -> Installation {
    let text = match fs::read_to_string(root.join(CONFIG_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Installation::NoStomme,
        Err(error) => return Installation::Broken(format!("could not read the file: {error}")),
    };

    match parse(&text) {
        Ok(selection) => Installation::Valid(selection),
        Err(reason) => Installation::Broken(reason),
    }
}

fn parse(text: &str) -> Result<Selection, String> {
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

    if table.contains_key("kind") {
        return Err(format!(
            "has `kind`, which schema {SCHEMA} does not read; declare `language`, `layers`, and `[roles]` instead"
        ));
    }

    let config: Config =
        toml::Value::Table(table)
            .try_into()
            .map_err(|error: toml::de::Error| {
                format!("does not match schema {SCHEMA}: {}", error.message())
            })?;
    let layers = layer::select(layer::catalog(), config.language, config.layers)
        .map_err(|error| error.to_string())?;

    Ok(Selection {
        language: config.language,
        layers,
        roles: config.roles,
    })
}

#[cfg(test)]
mod tests;
