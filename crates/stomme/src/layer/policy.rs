use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use crate::check::describe_toml_error;

pub(crate) type LintTable = BTreeMap<String, BTreeMap<String, LintValue>>;

#[derive(Debug)]
pub(crate) struct LintPolicy {
    lints: BTreeMap<String, Lint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Level {
    Allow,
    Warn,
    Deny,
    Forbid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Lint {
    pub(crate) level: Level,
    pub(crate) priority: i64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(
    untagged,
    expecting = "needs a lint level, or a table with `level` and an optional `priority`"
)]
pub(crate) enum LintValue {
    Level(Level),
    Table {
        level: Level,
        #[serde(default)]
        priority: i64,
    },
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequiredBan {
    pub(crate) path: String,
    pub(crate) reason: String,
}

#[derive(Deserialize)]
struct LintsFile {
    #[serde(default)]
    workspace: LintsWorkspace,
}

#[derive(Default, Deserialize)]
struct LintsWorkspace {
    #[serde(default)]
    lints: LintTable,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct BansFile {
    disallowed_methods: Vec<RequiredBan>,
}

pub(crate) fn parse_lint_policy(text: &str) -> Result<LintPolicy, String> {
    let file: LintsFile =
        toml::from_str(text).map_err(|error| describe_toml_error(&error, text))?;

    Ok(LintPolicy {
        lints: normalize(&file.workspace.lints),
    })
}

pub(crate) fn parse_required_bans(text: &str) -> Result<Vec<RequiredBan>, String> {
    let file: BansFile = toml::from_str(text).map_err(|error| describe_toml_error(&error, text))?;

    Ok(file.disallowed_methods)
}

pub(crate) fn normalize(tools: &LintTable) -> BTreeMap<String, Lint> {
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

impl LintPolicy {
    pub(crate) fn lints(&self) -> &BTreeMap<String, Lint> {
        &self.lints
    }
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
