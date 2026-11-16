use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;

use super::{Problem, Rule, describe_toml_error};
use crate::layer::policy::RequiredBan;

const CLIPPY_FILE: &str = "clippy.toml";

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ProjectFile {
    #[serde(default)]
    disallowed_methods: Vec<ProjectBan>,
}

#[derive(Deserialize)]
#[serde(
    untagged,
    expecting = "needs a method path, or a table with `path` and an optional `reason`"
)]
enum ProjectBan {
    Path(String),
    Table {
        path: String,
        reason: Option<String>,
    },
}

pub(super) fn check(
    root: &Path,
    required: &[(Rule, &RequiredBan)],
) -> anyhow::Result<Vec<Problem>> {
    if required.is_empty() {
        return Ok(Vec::new());
    }

    let entries = match read_project_bans(root)? {
        Ok(entries) => entries,
        Err(problem) => return Ok(vec![problem]),
    };
    let problems = required
        .iter()
        .filter_map(|&(rule, ban)| {
            ban_message(ban, &entries).map(|message| Problem {
                path: CLIPPY_FILE.to_string(),
                rule,
                message,
            })
        })
        .collect();

    Ok(problems)
}

fn read_project_bans(root: &Path) -> anyhow::Result<Result<Vec<ProjectBan>, Problem>> {
    let path = root.join(CLIPPY_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Ok(Vec::new())),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", path.display()));
        }
    };

    match toml::from_str::<ProjectFile>(&text) {
        Ok(file) => Ok(Ok(file.disallowed_methods)),
        Err(error) => Ok(Err(Problem {
            path: CLIPPY_FILE.to_string(),
            rule: Rule::RustConfigFile,
            message: format!("is not valid: {}", describe_toml_error(&error, &text)),
        })),
    }
}

fn ban_message(ban: &RequiredBan, entries: &[ProjectBan]) -> Option<String> {
    let reasons: Vec<Option<&str>> = entries
        .iter()
        .filter(|entry| entry.path() == ban.path)
        .map(ProjectBan::reason)
        .collect();

    if reasons.contains(&Some(ban.reason.as_str())) {
        return None;
    }

    let found = match reasons.first() {
        None => {
            return Some(format!(
                "lacks the `disallowed-methods` ban on `{}`; add it with reason \"{}\"",
                ban.path, ban.reason
            ));
        }
        Some(None) => "without a reason".to_string(),
        Some(Some(reason)) => format!("with reason \"{reason}\""),
    };

    Some(format!(
        "bans `{}` {found}; the shared policy needs reason \"{}\"",
        ban.path, ban.reason
    ))
}

impl ProjectBan {
    fn path(&self) -> &str {
        match self {
            Self::Path(path) | Self::Table { path, .. } => path,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Path(_) => None,
            Self::Table { reason, .. } => reason.as_deref(),
        }
    }
}

#[cfg(test)]
mod tests;
