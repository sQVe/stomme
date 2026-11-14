use std::fs;
use std::path::Path;

use anyhow::Context as _;
use serde_json::Value;

use super::{Problem, Rule};

const TAU_CONFIG: &str = ".pi/tau.json";

const REQUIRED_PARTS: &[&str] = &[
    "docs/README.md",
    "docs/development.md",
    "docs/adr/README.md",
    "docs/adr/TEMPLATE.md",
    "AGENTS.md",
    TAU_CONFIG,
    "renovate.json",
    "LICENSE",
];

pub(super) fn check(root: &Path) -> anyhow::Result<Vec<Problem>> {
    let mut problems: Vec<Problem> = REQUIRED_PARTS
        .iter()
        .filter(|part| !root.join(part).is_file())
        .map(|part| Problem {
            path: (*part).to_string(),
            rule: Rule::RequiredPart,
            message: "is missing; every stomme project needs this file".to_string(),
        })
        .collect();

    problems.extend(full_check_problem(root)?);

    Ok(problems)
}

fn full_check_problem(root: &Path) -> anyhow::Result<Option<Problem>> {
    let path = root.join(TAU_CONFIG);

    if !path.is_file() {
        return Ok(None);
    }

    let text =
        fs::read_to_string(&path).with_context(|| format!("Could not read {}", path.display()))?;
    let message = match serde_json::from_str::<Value>(&text) {
        Ok(config) if has_full_check_command(&config) => return Ok(None),
        Ok(_) => "needs `fullCheck.argv` as a non-empty array of strings".to_string(),
        Err(error) => format!("is not valid JSON: {error}"),
    };

    Ok(Some(Problem {
        path: TAU_CONFIG.to_string(),
        rule: Rule::TauFullCheck,
        message,
    }))
}

fn has_full_check_command(config: &Value) -> bool {
    config
        .pointer("/fullCheck/argv")
        .and_then(Value::as_array)
        .is_some_and(|argv| !argv.is_empty() && argv.iter().all(Value::is_string))
}

#[cfg(test)]
mod tests;
