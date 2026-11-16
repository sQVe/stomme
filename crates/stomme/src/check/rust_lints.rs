use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde::de::IgnoredAny;

use super::cargo_manifest::{ROOT_MANIFEST, read_manifest};
use super::{Problem, Rule, rust_members};
use crate::layer::policy::{Lint, LintPolicy, LintTable, normalize};

const ROOT_MISSING: &str =
    "is missing; a Rust project needs a root manifest with `[workspace.lints]`";

#[derive(Deserialize)]
struct Manifest {
    package: Option<IgnoredAny>,
    #[serde(default)]
    workspace: Workspace,
}

#[derive(Default, Deserialize)]
struct Workspace {
    #[serde(flatten)]
    members: rust_members::Workspace,
    #[serde(default)]
    lints: LintTable,
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
    )?);

    Ok(problems)
}

fn level_problems(policy: &LintPolicy, project: &BTreeMap<String, Lint>) -> Vec<Problem> {
    policy
        .lints()
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

#[cfg(test)]
mod tests;
