use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use super::Problem;
use super::target_branch::{self, TargetBranch};
use crate::rule::Rule;

mod diff;
mod forms;

#[cfg(test)]
mod tests;

enum Failure {
    Target(String),
    Other {
        path: Option<String>,
        reason: String,
    },
}

impl From<String> for Failure {
    fn from(reason: String) -> Self {
        Self::Other { path: None, reason }
    }
}

pub(super) fn check(root: &Path, target: &TargetBranch) -> Vec<Problem> {
    match scan(root, target) {
        Ok(problems) => problems,
        Err(Failure::Target(reason)) => vec![problem(
            &target.reference,
            format!("{reason}; fetch the target branch and try again"),
        )],
        Err(Failure::Other { path, reason }) => {
            vec![problem(
                path.as_deref().unwrap_or(&target.reference),
                reason,
            )]
        }
    }
}

fn problem(path: &str, message: String) -> Problem {
    Problem {
        path: path.to_string(),
        rule: Rule::WeakenedTest,
        message,
        key: None,
    }
}

fn scan(root: &Path, target: &TargetBranch) -> Result<Vec<Problem>, Failure> {
    let commit = match (target.resolve)(root, &target.reference) {
        Ok(commit) => commit,
        Err(reason) => {
            if !in_repository(root) {
                return Ok(Vec::new());
            }

            if has_no_commit(root)? {
                return Ok(Vec::new());
            }

            return Err(Failure::Target(reason));
        }
    };
    let prefix = diff::project_prefix(root)?;
    let mut problems = Vec::new();
    let mut base = Vec::new();
    let mut current = Vec::new();

    for path in diff::changed_paths(root, &commit)? {
        let text = diff::base_text(root, &commit, &prefix, &path.base)
            .map_err(|reason| file_failure(&path.text, reason))?;

        if let Some(text) = text {
            base.extend(forms::declarations(&text, path.language, &path.base));
        }

        let Some(text) = current_text(root, &path.text)? else {
            continue;
        };

        current.extend(forms::declarations(&text, path.language, &path.text));
        problems.extend(added_problems(root, &commit, &path)?);
    }

    problems.extend(deleted_problems(&base, &current, &target.reference));

    Ok(problems)
}

fn in_repository(root: &Path) -> bool {
    root.ancestors().any(|folder| folder.join(".git").exists())
}

fn has_no_commit(root: &Path) -> Result<bool, Failure> {
    let output = target_branch::run_git(root, &["rev-list", "--all", "--count"])
        .map_err(|error| repository_failure(format!("could not run `git`: {error}")))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        let reason = if detail.is_empty() {
            "`git rev-list --all --count` failed".to_string()
        } else {
            format!("`git rev-list --all --count` failed: {detail}")
        };

        return Err(repository_failure(reason));
    }

    let count = String::from_utf8_lossy(&output.stdout);

    Ok(count.trim() == "0")
}

fn file_failure(path: &str, reason: String) -> Failure {
    Failure::Other {
        path: Some(path.to_string()),
        reason,
    }
}

fn repository_failure(reason: String) -> Failure {
    Failure::Other { path: None, reason }
}

fn added_problems(
    root: &Path,
    commit: &str,
    path: &diff::ChangedPath,
) -> Result<Vec<Problem>, Failure> {
    let mut problems = Vec::new();

    let lines = diff::added_lines(root, commit, &path.base, &path.text)
        .map_err(|reason| file_failure(&path.text, reason))?;

    for line in lines {
        let Some((marker, form)) = forms::added_marker(&line.text, path.language) else {
            continue;
        };

        problems.push(problem(&path.text, form.message(line.line, marker)));
    }

    Ok(problems)
}

fn current_text(root: &Path, path: &str) -> Result<Option<String>, Failure> {
    match fs::read_to_string(root.join(path)) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(file_failure(path, format!("cannot read `{path}`: {error}"))),
    }
}

fn deleted_problems(
    base: &[forms::Declaration],
    current: &[forms::Declaration],
    reference: &str,
) -> Vec<Problem> {
    let present: BTreeSet<(&str, &str)> = current
        .iter()
        .map(|case| (case.path.as_str(), case.name.as_str()))
        .collect();
    let mut added: BTreeMap<&str, usize> = BTreeMap::new();

    for case in current {
        if !base.iter().any(|existing| same_case(existing, case)) {
            *added.entry(case.name.as_str()).or_default() += 1;
        }
    }

    let mut problems = Vec::new();

    for case in base {
        if present.contains(&(case.path.as_str(), case.name.as_str())) {
            continue;
        }

        let available = added.entry(case.name.as_str()).or_default();

        if *available > 0 {
            *available -= 1;
            continue;
        }

        problems.push(deleted_problem(case, reference));
    }

    problems
}

fn same_case(left: &forms::Declaration, right: &forms::Declaration) -> bool {
    left.path == right.path && left.name == right.name
}

fn deleted_problem(case: &forms::Declaration, reference: &str) -> Problem {
    problem(
        &case.path,
        format!(
            "deletes the test case `{}` that `{reference}` declares on line {}; keep the case",
            case.name, case.line
        ),
    )
}
