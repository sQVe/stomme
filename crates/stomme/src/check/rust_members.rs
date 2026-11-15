use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;

use super::cargo_manifest::{ROOT_MANIFEST, config_problem, read_manifest};
use super::{Problem, Rule};

const MEMBER_MISSING: &str = "is missing; `workspace.members` names this folder as a member";

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

pub(super) fn check(
    root: &Path,
    root_package: bool,
    patterns: &[String],
    exclude: &[String],
) -> anyhow::Result<Vec<Problem>> {
    let mut problems = Vec::new();
    let mut members = BTreeSet::new();

    if root_package {
        members.insert(String::new());
    }

    for pattern in patterns {
        match expand(root, pattern)? {
            Ok(paths) => members.extend(paths),
            Err(problem) => problems.push(problem),
        }
    }

    let excluded: Vec<String> = exclude
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

#[cfg(test)]
mod tests;
