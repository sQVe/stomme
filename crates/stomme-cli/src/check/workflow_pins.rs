use std::fs;
use std::path::Path;

use anyhow::Context as _;

use super::Problem;
use crate::rule::Rule;

const WORKFLOWS_FOLDER: &str = ".github/workflows";
const WORKFLOWS_PREFIX: &str = "sqve/stomme/.github/workflows/";
const OWNER: &str = "sQVe/stomme";

pub(super) fn check(root: &Path, version: &str) -> anyhow::Result<Vec<Problem>> {
    let mut problems = Vec::new();

    for name in workflow_names(root)? {
        let path = root.join(WORKFLOWS_FOLDER).join(&name);
        let repository_path = format!("{WORKFLOWS_FOLDER}/{name}");
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;

        for (index, line) in text.lines().enumerate() {
            let problem = call_problem(line, index + 1, version, &repository_path);

            problems.extend(problem);
        }
    }

    Ok(problems)
}

fn workflow_names(root: &Path) -> anyhow::Result<Vec<String>> {
    let github = root.join(".github");

    if is_symlink(&github) || !github.is_dir() {
        return Ok(Vec::new());
    }

    let folder = github.join("workflows");

    if is_symlink(&folder) || !folder.is_dir() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();

    for entry in fs::read_dir(&folder).with_context(|| format!("read {}", folder.display()))? {
        let entry = entry.with_context(|| format!("read an entry of {}", folder.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("read the file type of {}", entry.path().display()))?;

        if !file_type.is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();

        if is_workflow_name(&name) {
            names.push(name);
        }
    }

    names.sort();

    Ok(names)
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
}

fn is_workflow_name(name: &str) -> bool {
    name.ends_with(".yml") || name.ends_with(".yaml")
}

fn call_problem(line: &str, number: usize, version: &str, path: &str) -> Option<Problem> {
    let trimmed = line.trim_start();
    let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    let rest = trimmed.strip_prefix("uses:")?;
    let (value, quoted) = unquote(rest.trim());
    let (called, after) = value.split_once('@').unwrap_or((value, ""));

    if !called.to_ascii_lowercase().starts_with(WORKFLOWS_PREFIX) {
        return None;
    }

    let workflow = called.rsplit('/').next().unwrap_or(called);
    let owner = called.split('/').take(2).collect::<Vec<_>>().join("/");
    let (reference, comment) = match after.split_once(char::is_whitespace) {
        Some((reference, comment)) => (reference, comment.trim()),
        None => (after, ""),
    };

    if quoted {
        return Some(problem(
            path,
            format!(
                "line {number}: quotes the call to `{workflow}`; write it unquoted so Renovate can move it"
            ),
        ));
    }

    if owner != OWNER {
        return Some(problem(
            path,
            format!(
                "line {number}: calls `{workflow}` through `{owner}`; write `{OWNER}` so Renovate can move it"
            ),
        ));
    }

    if !is_commit(reference) {
        return Some(problem(
            path,
            format!(
                "line {number}: calls `{workflow}` by the moving name `{reference}`; pin the release commit of v{version} with `# v{version}`"
            ),
        ));
    }

    if !comment.starts_with('#') {
        return Some(problem(
            path,
            format!(
                "line {number}: pins `{workflow}` at a commit with no version comment; add `# v{version}`"
            ),
        ));
    }

    let Some(pinned) = comment_version(comment) else {
        return Some(problem(
            path,
            format!(
                "line {number}: pins `{workflow}` with the comment `{comment}`; pin a release commit with `# v{version}`"
            ),
        ));
    };

    if pinned != version {
        return Some(problem(
            path,
            format!(
                "line {number}: pins `{workflow}` at v{pinned}; `stomme.toml` has version {version}"
            ),
        ));
    }

    None
}

fn unquote(value: &str) -> (&str, bool) {
    for quote in ['"', '\''] {
        let quoted = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote));

        if let Some(inner) = quoted {
            return (inner, true);
        }
    }

    (value, false)
}

fn is_commit(reference: &str) -> bool {
    let is_lowercase_hex = |byte: u8| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte);

    reference.len() == 40 && reference.bytes().all(is_lowercase_hex)
}

fn comment_version(comment: &str) -> Option<&str> {
    let rest = comment.strip_prefix('#')?;
    let rest = rest.trim_start();
    let version = rest.strip_prefix('v')?.trim_end();

    if is_version(version) {
        Some(version)
    } else {
        None
    }
}

fn is_version(text: &str) -> bool {
    let (core, suffix) = match text.split_once('-') {
        Some((core, suffix)) => (core, Some(suffix)),
        None => (text, None),
    };

    if core.is_empty()
        || !core
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
    {
        return false;
    }

    let suffix_is_valid = |suffix: &str| {
        !suffix.is_empty()
            && suffix.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-')
            })
    };

    suffix.is_none_or(suffix_is_valid)
}

fn problem(path: &str, message: String) -> Problem {
    Problem {
        path: path.to_string(),
        rule: Rule::WorkflowPin,
        message,
        key: None,
    }
}

#[cfg(test)]
mod tests;
