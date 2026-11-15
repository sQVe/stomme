use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::Deserialize;
use serde::de::IgnoredAny;

use super::cargo_manifest::{ROOT_MANIFEST, read_manifest};
use super::{Problem, Rule, describe_toml_error, rust_members};

const ROOT_MISSING: &str =
    "is missing; a Rust project needs a root manifest with `[workspace.lints]`";

#[derive(Debug)]
pub(crate) struct LintPolicy {
    lints: BTreeMap<String, Lint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Allow,
    Warn,
    Deny,
    Forbid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Lint {
    level: Level,
    priority: i64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(
    untagged,
    expecting = "needs a lint level, or a table with `level` and an optional `priority`"
)]
enum LintValue {
    Level(Level),
    Table {
        level: Level,
        #[serde(default)]
        priority: i64,
    },
}

#[derive(Deserialize)]
struct Manifest {
    package: Option<IgnoredAny>,
    #[serde(default)]
    workspace: Workspace,
}

#[derive(Default, Deserialize)]
struct Workspace {
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    lints: BTreeMap<String, BTreeMap<String, LintValue>>,
}

pub(crate) fn parse_policy(text: &str) -> Result<LintPolicy, String> {
    let manifest: Manifest =
        toml::from_str(text).map_err(|error| describe_toml_error(&error, text))?;

    Ok(LintPolicy {
        lints: normalize(&manifest.workspace.lints),
    })
}

pub(super) fn check(root: &Path, policy: &LintPolicy) -> anyhow::Result<Vec<Problem>> {
    let manifest: Manifest = match read_manifest(root, ROOT_MANIFEST, ROOT_MISSING)? {
        Ok(manifest) => manifest,
        Err(problem) => return Ok(vec![problem]),
    };
    let lints = normalize(&manifest.workspace.lints);
    let mut problems = level_problems(policy, &lints);

    problems.extend(rust_members::check(
        root,
        manifest.package.is_some(),
        &manifest.workspace.members,
        &manifest.workspace.exclude,
    )?);

    Ok(problems)
}

fn level_problems(policy: &LintPolicy, project: &BTreeMap<String, Lint>) -> Vec<Problem> {
    policy
        .lints
        .iter()
        .filter_map(|(name, required)| weakening(name, *required, project.get(name).copied()))
        .map(|message| Problem {
            path: ROOT_MANIFEST.to_string(),
            rule: Rule::RustLintLevel,
            message,
        })
        .collect()
}

fn weakening(name: &str, required: Lint, found: Option<Lint>) -> Option<String> {
    let Some(found) = found else {
        return Some(format!(
            "lacks lint `{name}` in `[workspace.lints]`; the shared policy sets it to {required}"
        ));
    };

    if found.level < required.level {
        return Some(format!(
            "sets lint `{name}` to `{}`; the shared policy needs at least `{}`",
            found.level, required.level
        ));
    }

    if found.priority != required.priority {
        return Some(format!(
            "sets lint `{name}` to priority {}; the shared policy uses priority {}",
            found.priority, required.priority
        ));
    }

    None
}

fn normalize(tools: &BTreeMap<String, BTreeMap<String, LintValue>>) -> BTreeMap<String, Lint> {
    let mut lints = BTreeMap::new();

    for (tool, values) in tools {
        for (name, value) in values {
            let lint = match *value {
                LintValue::Level(level) => Lint { level, priority: 0 },
                LintValue::Table { level, priority } => Lint { level, priority },
            };
            let name = if tool == "rust" {
                name.clone()
            } else {
                format!("{tool}::{name}")
            };

            lints.insert(name, lint);
        }
    }

    lints
}

impl fmt::Display for Level {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Allow => "allow",
            Self::Warn => "warn",
            Self::Deny => "deny",
            Self::Forbid => "forbid",
        };

        formatter.write_str(name)
    }
}

impl fmt::Display for Lint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` with priority {}",
            self.level, self.priority
        )
    }
}

#[cfg(test)]
mod tests;
