use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Serialize;

use super::assets::{self, Source};
use super::{InitError, Request, merge};
use crate::config::{CONFIG_FILE, SCHEMA};
use crate::layer::{self, Catalog, Language};

const CONFIG_OWNER: &str = "init";
const KEPT_ENTRY: &str = ".git";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub(crate) struct InitPlan {
    pub(super) files: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct Config<'a> {
    schema: i64,
    version: &'static str,
    language: Language,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    layers: BTreeSet<&'a str>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    roles: BTreeMap<&'a str, String>,
}

pub(crate) fn validate(root: &Path, catalog: &Catalog, request: &Request) -> Result<(), InitError> {
    layer::select(catalog, request.language, request.layers.clone())?;
    validate_roles(request)?;
    check_empty(root)
}

pub(crate) fn plan(request: &Request, sources: &[&Source]) -> Result<InitPlan, InitError> {
    let mut owners: BTreeMap<String, &str> = BTreeMap::new();
    let mut files = BTreeMap::new();
    let mut fragments: BTreeMap<String, Vec<(&str, String)>> = BTreeMap::new();

    owners.insert(CONFIG_FILE.to_string(), CONFIG_OWNER);
    files.insert(CONFIG_FILE.to_string(), config_text(request)?);

    for source in sources {
        for (path, text) in source.files {
            let path = substitute(request, path);

            if let Some(first) = owners.insert(path.clone(), source.owner) {
                return Err(owner_clash(path, first, source.owner));
            }

            files.insert(path, substitute(request, text));
        }

        for (path, text) in source.merges {
            fragments
                .entry(substitute(request, path))
                .or_default()
                .push((source.owner, substitute(request, text)));
        }
    }

    for (path, parts) in fragments {
        if let Some(first) = owners.get(&path) {
            return Err(owner_clash(path, first, parts[0].0));
        }

        let borrowed: Vec<(&str, &str)> = parts
            .iter()
            .map(|(owner, text)| (*owner, text.as_str()))
            .collect();
        let text = merge::merge(&path, &borrowed)?;

        files.insert(path, text);
    }

    Ok(InitPlan { files })
}

pub(crate) fn execute(root: &Path, plan: &InitPlan) -> Result<(), InitError> {
    for (path, text) in &plan.files {
        let target = root.join(path);
        let write_error = |source| InitError::Write {
            path: path.clone(),
            source,
        };

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(write_error)?;
        }

        fs::write(&target, text).map_err(write_error)?;
    }

    Ok(())
}

fn validate_roles(request: &Request) -> Result<(), InitError> {
    let mut seen = BTreeSet::new();

    for role in &request.roles {
        if assets::role(request.language, role).is_none() {
            return Err(InitError::UnknownRole {
                role: role.clone(),
                language: request.language,
            });
        }

        if !seen.insert(role) {
            return Err(InitError::DuplicateRole { role: role.clone() });
        }
    }

    Ok(())
}

fn check_empty(root: &Path) -> Result<(), InitError> {
    let read_error = |source| InitError::Read {
        path: root.display().to_string(),
        source,
    };

    for entry in fs::read_dir(root).map_err(read_error)? {
        let entry = entry.map_err(read_error)?;

        if entry.file_name() != KEPT_ENTRY {
            return Err(InitError::NotEmpty {
                path: root.display().to_string(),
            });
        }
    }

    Ok(())
}

fn config_text(request: &Request) -> Result<String, InitError> {
    let mut roles = BTreeMap::new();

    for role in &request.roles {
        roles.insert(role.as_str(), role_path(request, role)?);
    }

    let config = Config {
        schema: SCHEMA,
        version: VERSION,
        language: request.language,
        layers: request.layers.iter().map(String::as_str).collect(),
        roles,
    };

    Ok(toml::to_string(&config).expect("the init config always serializes"))
}

fn role_path(request: &Request, role: &str) -> Result<String, InitError> {
    match request.language {
        Language::Rust => Ok(format!("crates/{}-{role}", request.name)),
        Language::TypeScript => Err(InitError::UnknownRole {
            role: role.to_string(),
            language: request.language,
        }),
    }
}

fn substitute(request: &Request, text: &str) -> String {
    text.replace("{{name}}", &request.name)
        .replace("{{version}}", VERSION)
}

fn owner_clash(path: String, first: &str, second: &str) -> InitError {
    InitError::OwnerClash {
        path,
        first: first.to_string(),
        second: second.to_string(),
    }
}
