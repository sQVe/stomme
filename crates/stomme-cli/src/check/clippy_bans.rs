use std::path::Path;

use serde::Deserialize;

use super::cargo_manifest::{config_problem, read_toml_file};
use super::{Problem, Rule};
use crate::layer::policy::RequiredBan;

const CLIPPY_FILE: &str = "clippy.toml";
const CLIPPY_FILE_NAMES: [&str; 2] = [".clippy.toml", CLIPPY_FILE];

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ProjectFile {
    #[serde(default)]
    disallowed_methods: Vec<ProjectBan>,
}

#[derive(Deserialize)]
#[serde(
    untagged,
    expecting = "needs a method path, or a table with `path`, an optional `reason`, and an optional `replacement`"
)]
enum ProjectBan {
    Path(String),
    Table {
        path: String,
        reason: Option<String>,
        #[serde(rename = "replacement")]
        _replacement: Option<String>,
    },
}

pub(super) fn check(
    root: &Path,
    required: &[(Rule, &RequiredBan)],
) -> anyhow::Result<Vec<Problem>> {
    if required.is_empty() {
        return Ok(Vec::new());
    }

    let (path, entries) = match read_project_bans(root)? {
        Ok(found) => found,
        Err(problem) => return Ok(vec![problem]),
    };
    let problems = required
        .iter()
        .filter_map(|&(rule, ban)| {
            ban_message(ban, &entries).map(|message| Problem {
                path: path.clone(),
                rule,
                message,
                key: None,
            })
        })
        .collect();

    Ok(problems)
}

fn read_project_bans(root: &Path) -> anyhow::Result<Result<(String, Vec<ProjectBan>), Problem>> {
    for name in CLIPPY_FILE_NAMES {
        if !root.join(name).is_file() {
            continue;
        }

        let bans = match read_toml_file::<ProjectFile>(root, name)? {
            Some(Ok(file)) => file.disallowed_methods,
            Some(Err(reason)) => {
                let message = format!("is not valid: {reason}");

                return Ok(Err(config_problem(name.to_string(), message)));
            }
            None => continue,
        };

        return Ok(Ok((name.to_string(), bans)));
    }

    Ok(Ok((CLIPPY_FILE.to_string(), Vec::new())))
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
