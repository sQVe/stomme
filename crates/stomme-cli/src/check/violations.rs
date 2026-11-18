use std::collections::BTreeSet;
use std::path::Path;

use super::Problem;
use crate::config::{CONFIG_FILE, Violation};
use crate::rule::Rule;

mod target_branch;

pub(crate) use target_branch::{TargetBranch, TargetRead, default_reference};

pub(crate) fn apply(problems: Vec<Problem>, recorded: &[Violation]) -> Vec<Problem> {
    let mut matched = vec![false; recorded.len()];
    let mut remaining = Vec::new();

    for problem in problems {
        let Some(key) = problem.key.as_ref() else {
            remaining.push(problem);
            continue;
        };
        let found = recorded.iter().position(|violation| {
            violation.path == problem.path
                && violation.rule == problem.rule
                && violation.key == *key
        });

        match found {
            Some(index) => matched[index] = true,
            None => remaining.push(problem),
        }
    }

    for (index, violation) in recorded.iter().enumerate() {
        if !matched[index] {
            remaining.push(stale_problem(violation));
        }
    }

    remaining
}

fn stale_problem(violation: &Violation) -> Problem {
    Problem {
        path: CONFIG_FILE.to_string(),
        rule: Rule::ViolationStale,
        message: format!(
            "records rule `{}` at `{}` with key `{}`, but no problem matches; remove the entry",
            violation.rule.id(),
            violation.path,
            violation.key
        ),
        key: None,
    }
}

pub(crate) fn growth(root: &Path, recorded: &[Violation], target: &TargetBranch) -> Vec<Problem> {
    if recorded.is_empty() {
        return Vec::new();
    }

    match (target.read)(root, &target.reference) {
        TargetRead::Unavailable(reason) => vec![growth_problem(format!(
            "cannot read `stomme.toml` from `{}`: {reason}; fetch `{}` and try again",
            target.reference, target.reference
        ))],
        TargetRead::NoConfig => Vec::new(),
        TargetRead::Config(text) => growth_against(recorded, target, &text),
    }
}

fn growth_against(recorded: &[Violation], target: &TargetBranch, text: &str) -> Vec<Problem> {
    let existing = match target_entries(text) {
        Ok(existing) => existing,
        Err(()) => {
            return vec![growth_problem(format!(
                "cannot read the `violations` list from `stomme.toml` on `{}`",
                target.reference
            ))];
        }
    };

    recorded
        .iter()
        .filter(|violation| !existing.contains(&entry_key(violation)))
        .map(|violation| {
            growth_problem(format!(
                "records rule `{}` at `{}` with key `{}`, which `{}` does not record; fix the break instead of recording it",
                violation.rule.id(),
                violation.path,
                violation.key,
                target.reference
            ))
        })
        .collect()
}

fn entry_key(violation: &Violation) -> (String, String, String) {
    (
        violation.path.clone(),
        violation.rule.id().to_string(),
        violation.key.clone(),
    )
}

fn target_entries(text: &str) -> Result<BTreeSet<(String, String, String)>, ()> {
    let table: toml::Table = text.parse().map_err(|_| ())?;
    let Some(value) = table.get("violations") else {
        return Ok(BTreeSet::new());
    };
    let entries = value.as_array().ok_or(())?;
    let mut keys = BTreeSet::new();

    for entry in entries {
        let table = entry.as_table().ok_or(())?;
        let path = table.get("path").and_then(toml::Value::as_str).ok_or(())?;
        let rule = table.get("rule").and_then(toml::Value::as_str).ok_or(())?;
        let key = table.get("key").and_then(toml::Value::as_str).ok_or(())?;

        keys.insert((path.to_string(), rule.to_string(), key.to_string()));
    }

    Ok(keys)
}

fn growth_problem(message: String) -> Problem {
    Problem {
        path: CONFIG_FILE.to_string(),
        rule: Rule::ViolationGrowth,
        message,
        key: None,
    }
}

#[cfg(test)]
mod tests;
