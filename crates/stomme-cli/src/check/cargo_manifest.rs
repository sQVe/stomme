use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use std::collections::BTreeMap;

use anyhow::Context as _;
use serde::Deserialize;
use serde::de::{DeserializeOwned, Error as _, IgnoredAny, MapAccess, Visitor};
use toml_datetime::de::VisitMap;

use super::{Problem, Rule, describe_toml_error};
use crate::layer::policy::LintTable;

pub(super) const ROOT_MANIFEST: &str = "Cargo.toml";
const ROOT_MISSING: &str =
    "is missing; a Rust project needs a root manifest with `[workspace.lints]`";

pub(super) type Dependencies = BTreeMap<String, Dependency>;

#[derive(Deserialize)]
pub(super) struct RootManifest {
    package: Option<IgnoredAny>,
    #[serde(default)]
    pub(super) workspace: Workspace,
}

#[derive(Default, Deserialize)]
pub(super) struct Workspace {
    #[serde(default)]
    pub(super) members: Vec<String>,
    #[serde(default)]
    pub(super) exclude: Vec<String>,
    #[serde(default)]
    pub(super) dependencies: Dependencies,
    #[serde(default)]
    pub(super) lints: LintTable,
}

pub(super) enum Dependency {
    Table {
        path: Option<String>,
        workspace: bool,
    },
    Version,
}

impl<'de> Deserialize<'de> for Dependency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(DependencyVisitor)
    }
}

struct DependencyVisitor;

impl<'de> Visitor<'de> for DependencyVisitor {
    type Value = Dependency;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a version string or a dependency table")
    }

    fn visit_str<E>(self, _version: &str) -> Result<Dependency, E>
    where
        E: serde::de::Error,
    {
        Ok(Dependency::Version)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Dependency, A::Error>
    where
        A: MapAccess<'de>,
    {
        let first = match VisitMap::next_key_seed(&mut map)? {
            Some(VisitMap::Key(key)) => key.into_owned(),
            // A datetime reaches Serde as a map with one private key, so a map that continues past
            // that key is a table that holds a user key of the same name.
            Some(VisitMap::Datetime(_)) => match map.next_key::<String>()? {
                Some(key) => key,
                None => {
                    return Err(A::Error::custom(
                        "a TOML date or timestamp is not a dependency",
                    ));
                }
            },
            None => {
                return Ok(Dependency::Table {
                    path: None,
                    workspace: false,
                });
            }
        };

        read_table(&mut map, first)
    }
}

fn read_table<'de, A>(map: &mut A, first: String) -> Result<Dependency, A::Error>
where
    A: MapAccess<'de>,
{
    let mut path = None;
    let mut workspace = false;
    let mut key = first;

    loop {
        match key.as_str() {
            "path" => path = Some(map.next_value()?),
            "version" => {
                map.next_value::<String>()?;
            }
            "workspace" => workspace = map.next_value()?,
            _ => {
                map.next_value::<IgnoredAny>()?;
            }
        }

        match map.next_key::<String>()? {
            Some(next) => key = next,
            None => break,
        }
    }

    Ok(Dependency::Table { path, workspace })
}

pub(super) fn read_root(root: &Path) -> anyhow::Result<Result<RootManifest, Problem>> {
    read_manifest(root, ROOT_MANIFEST, ROOT_MISSING)
}

pub(super) fn read_manifest<T: DeserializeOwned>(
    root: &Path,
    path: &str,
    missing: &str,
) -> anyhow::Result<Result<T, Problem>> {
    let message = match read_toml_file(root, path)? {
        Some(Ok(manifest)) => return Ok(Ok(manifest)),
        Some(Err(reason)) => format!("is not a valid manifest: {reason}"),
        None => missing.to_string(),
    };

    Ok(Err(config_problem(path.to_string(), message)))
}

// Returns `None` for a missing file, so each caller decides what a missing file means.
pub(super) fn read_toml_file<T: DeserializeOwned>(
    root: &Path,
    path: &str,
) -> anyhow::Result<Option<Result<T, String>>> {
    let full_path = root.join(path);
    let text = match fs::read_to_string(&full_path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return Ok(Some(Err("the file is not valid UTF-8".to_string())));
        }
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", full_path.display()));
        }
    };
    let parsed = toml::from_str(&text).map_err(|error| describe_toml_error(&error, &text));

    Ok(Some(parsed))
}

pub(super) fn config_problem(path: String, message: String) -> Problem {
    Problem {
        path,
        rule: Rule::RustConfigFile,
        message,
        key: None,
    }
}

impl RootManifest {
    pub(super) fn has_package(&self) -> bool {
        self.package.is_some()
    }
}

impl Dependency {
    pub(super) fn path(&self) -> Option<&str> {
        match self {
            Self::Version => None,
            Self::Table { path, .. } => path.as_deref(),
        }
    }

    pub(super) fn inherits(&self) -> bool {
        matches!(
            self,
            Self::Table {
                workspace: true,
                ..
            }
        )
    }
}
