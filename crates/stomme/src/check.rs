use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use crate::config::{self, CONFIG_FILE, Installation};
use crate::layer::{Catalog, Language};
use crate::rule::Rule;

mod adr_index;
mod cargo_manifest;
mod clippy_bans;
mod required_parts;
mod rust_lints;
mod rust_members;

const ADR_FOLDER: &str = "docs/adr";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) path: String,
    pub(crate) rule: Rule,
    pub(crate) message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} [{}]",
            self.path,
            self.message,
            self.rule.id()
        )
    }
}

pub(crate) fn run(root: &Path, catalog: &Catalog) -> anyhow::Result<Vec<Problem>> {
    let installation = config::read(root, catalog);
    let mut problems: Vec<Problem> = installation_problem(&installation).into_iter().collect();

    problems.extend(required_parts::check(root)?);
    problems.extend(adr_index::check(root, ADR_FOLDER)?);

    if let Installation::Valid(selection) = &installation
        && selection.language == Language::Rust
    {
        problems.extend(rust_problems(root, catalog, &selection.layers)?);
    }

    Ok(problems)
}

fn rust_problems(
    root: &Path,
    catalog: &Catalog,
    layers: &BTreeSet<String>,
) -> anyhow::Result<Vec<Problem>> {
    let mut problems = Vec::new();

    match cargo_manifest::read_root(root)? {
        Ok(manifest) => {
            problems.extend(rust_lints::check(&manifest, catalog.rust_lints()));
            problems.extend(rust_members::check(root, &manifest)?);
        }
        Err(problem) => problems.push(problem),
    }

    problems.extend(clippy_bans::check(root, &catalog.required_bans(layers))?);

    Ok(problems)
}

pub(crate) fn describe_toml_error(error: &toml::de::Error, text: &str) -> String {
    match error.span() {
        Some(span) => {
            let line = text[..span.start].matches('\n').count() + 1;

            format!("line {line}: {}", error.message())
        }
        None => error.message().to_string(),
    }
}

fn installation_problem(installation: &Installation) -> Option<Problem> {
    let (rule, message) = match installation {
        Installation::Valid(_) => return None,
        Installation::NoStomme => (
            Rule::NoStomme,
            "is missing; add one with `schema = 1` and a `language`".to_string(),
        ),
        Installation::Broken(reason) => (Rule::BrokenInstallation, reason.clone()),
    };

    Some(Problem {
        path: CONFIG_FILE.to_string(),
        rule,
        message,
    })
}

#[cfg(test)]
mod tests;
