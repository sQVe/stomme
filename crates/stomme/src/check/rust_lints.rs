use std::collections::BTreeMap;

use super::cargo_manifest::{ROOT_MANIFEST, RootManifest};
use super::{Problem, Rule};
use crate::layer::policy::{Lint, LintPolicy, normalize};

pub(super) fn check(manifest: &RootManifest, policy: &LintPolicy) -> Vec<Problem> {
    let lints = normalize(&manifest.workspace.lints);

    level_problems(policy, &lints)
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
