use std::fs;
use std::io;
use std::path::Path;

use std::collections::BTreeMap;

use anyhow::Context as _;
use serde::Deserialize;
use serde::de::{DeserializeOwned, IgnoredAny};

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

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Dependency {
    Table {
        path: Option<String>,
        #[serde(default)]
        workspace: bool,
    },
    Version(IgnoredAny),
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
            Self::Version(_) => None,
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
