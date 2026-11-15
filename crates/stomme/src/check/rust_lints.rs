use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;
use serde::de::{DeserializeOwned, IgnoredAny};

use super::{Problem, Rule, describe_toml_error};

const ROOT_MANIFEST: &str = "Cargo.toml";
const ROOT_MISSING: &str =
    "is missing; a Rust project needs a root manifest with `[workspace.lints]`";
const MEMBER_MISSING: &str = "is missing; `workspace.members` names this folder as a member";

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

#[derive(Deserialize)]
struct MemberManifest {
    #[serde(default)]
    lints: MemberLints,
}

#[derive(Default, Deserialize)]
struct MemberLints {
    #[serde(default)]
    workspace: bool,
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

    problems.extend(member_problems(root, &manifest)?);

    Ok(problems)
}

fn read_manifest<T: DeserializeOwned>(
    root: &Path,
    path: &str,
    missing: &str,
) -> anyhow::Result<Result<T, Problem>> {
    let full_path = root.join(path);
    let message = match fs::read_to_string(&full_path) {
        Ok(text) => match toml::from_str(&text) {
            Ok(manifest) => return Ok(Ok(manifest)),
            Err(error) => format!(
                "is not a valid manifest: {}",
                describe_toml_error(&error, &text)
            ),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => missing.to_string(),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", full_path.display()));
        }
    };

    Ok(Err(config_problem(path.to_string(), message)))
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

fn member_problems(root: &Path, manifest: &Manifest) -> anyhow::Result<Vec<Problem>> {
    let mut problems = Vec::new();
    let mut members = BTreeSet::new();

    if manifest.package.is_some() {
        members.insert(String::new());
    }

    for pattern in &manifest.workspace.members {
        match expand(root, pattern)? {
            Ok(paths) => members.extend(paths),
            Err(problem) => problems.push(problem),
        }
    }

    let excluded: Vec<String> = manifest
        .workspace
        .exclude
        .iter()
        .map(|path| segments(path).join("/"))
        .collect();

    for member in members
        .iter()
        .filter(|member| !is_excluded(member, &excluded))
    {
        problems.extend(inheritance_problem(root, member)?);
    }

    Ok(problems)
}

fn expand(root: &Path, pattern: &str) -> anyhow::Result<Result<Vec<String>, Problem>> {
    let segments = segments(pattern);

    if segments.iter().any(|segment| is_unsupported_glob(segment)) {
        return Ok(Err(config_problem(
            ROOT_MANIFEST.to_string(),
            format!(
                "lists member `{pattern}`; stomme expands only literal paths and `*` inside one path segment"
            ),
        )));
    }

    let mut paths = vec![String::new()];

    for segment in segments {
        if segment.contains('*') {
            let mut matched = Vec::new();

            for parent in &paths {
                matched.extend(matching_folders(root, parent, segment)?);
            }

            paths = matched;
        } else {
            paths = paths.iter().map(|parent| join(parent, segment)).collect();
        }
    }

    Ok(Ok(paths))
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect()
}

fn is_unsupported_glob(segment: &str) -> bool {
    segment.contains("**") || segment.contains(['?', '[', ']', '{', '}'])
}

fn matching_folders(root: &Path, parent: &str, pattern: &str) -> anyhow::Result<Vec<String>> {
    let folder = root.join(parent);
    let mut matched = Vec::new();

    if !folder.is_dir() {
        return Ok(matched);
    }

    let entries =
        fs::read_dir(&folder).with_context(|| format!("Could not read {}", folder.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| format!("Could not read {}", folder.display()))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };

        if entry.path().is_dir() && matches_segment(pattern, name) {
            matched.push(join(parent, name));
        }
    }

    Ok(matched)
}

fn matches_segment(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let [first, middle @ .., last] = parts.as_slice() else {
        unreachable!("`expand` matches only a segment that contains `*`");
    };
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };

    for part in middle {
        let Some(start) = rest.find(part) else {
            return false;
        };

        rest = &rest[start + part.len()..];
    }

    rest.ends_with(last)
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

fn is_excluded(member: &str, excluded: &[String]) -> bool {
    excluded
        .iter()
        .any(|path| member == path || member.starts_with(&format!("{path}/")))
}

fn inheritance_problem(root: &Path, member: &str) -> anyhow::Result<Option<Problem>> {
    let path = join(member, ROOT_MANIFEST);
    let manifest: MemberManifest = match read_manifest(root, &path, MEMBER_MISSING)? {
        Ok(manifest) => manifest,
        Err(problem) => return Ok(Some(problem)),
    };

    if manifest.lints.workspace {
        return Ok(None);
    }

    Ok(Some(Problem {
        path,
        rule: Rule::RustLintInheritance,
        message:
            "lacks `[lints] workspace = true`; every workspace member inherits the shared lints"
                .to_string(),
    }))
}

fn config_problem(path: String, message: String) -> Problem {
    Problem {
        path,
        rule: Rule::RustConfigFile,
        message,
    }
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
