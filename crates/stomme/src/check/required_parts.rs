use std::path::Path;

use anyhow::Context as _;

use super::Problem;
use crate::full_check::{self, FullCheckError};
use crate::rule::Rule;

const REQUIRED_PARTS: &[&str] = &[
    "docs/README.md",
    "docs/development.md",
    "docs/adr/README.md",
    "docs/adr/TEMPLATE.md",
    "AGENTS.md",
    full_check::TAU_CONFIG,
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
    match full_check::read(root) {
        Ok(_) => Ok(None),
        Err(FullCheckError::Read { path, source }) => {
            Err(source).with_context(|| format!("Could not read {path}"))
        }
        Err(error) => Ok(Some(Problem {
            path: full_check::TAU_CONFIG.to_string(),
            rule: Rule::TauFullCheck,
            message: error.to_string(),
        })),
    }
}

#[cfg(test)]
mod tests;
