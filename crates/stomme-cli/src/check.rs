use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use crate::config::{self, CONFIG_FILE, Installation};
use crate::layer::{Catalog, Language};
use crate::rule::Rule;

mod adr_index;
mod cargo_lock;
mod cargo_manifest;
mod clippy_bans;
mod layer_use;
mod layout;
mod modules;
mod package_boundaries;
mod package_json;
mod package_pins;
mod required_parts;
mod roles;
mod rust_lints;
mod rust_members;
mod source_files;
mod synced_files;
mod target_branch;
#[cfg(test)]
mod test_support;
mod violations;
mod weakened;
mod workflow_pins;

pub(crate) use target_branch::{TargetBranch, default_reference};

#[cfg(test)]
pub(crate) use package_pins::write_pinned_fixture;

const ADR_FOLDER: &str = "docs/adr";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) path: String,
    pub(crate) rule: Rule,
    pub(crate) message: String,
    pub(crate) key: Option<String>,
}

impl fmt::Display for Problem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = match &self.key {
            Some(key) => format!(" (key `{key}`)"),
            None => String::new(),
        };

        write!(
            formatter,
            "{}: {}{} [{}]",
            self.path,
            self.message,
            key,
            self.rule.id()
        )
    }
}

pub(crate) fn run(
    root: &Path,
    catalog: &Catalog,
    target: &TargetBranch,
    file: &config::FileRead,
) -> anyhow::Result<Vec<Problem>> {
    let installation = config::read(root, catalog, file);
    let mut problems: Vec<Problem> = installation_problem(&installation).into_iter().collect();
    let selection = match &installation {
        Installation::Valid(selection) => Some(selection),
        Installation::NoStomme | Installation::Broken(_) => None,
    };

    if let Some(selection) = selection {
        problems.extend(synced_files::check(root, selection, catalog)?);
    }

    problems.extend(required_parts::check(root)?);
    let package = package_json::read(root)?;
    problems.extend(package_pins::check(package.as_ref()));
    problems.extend(adr_index::check(root, ADR_FOLDER)?);
    let package_value = package.as_ref().and_then(|result| result.as_ref().ok());

    if let Some(selection) = selection {
        if selection.language == Language::Rust {
            problems.extend(rust_problems(root, catalog, &selection.layers)?);
            problems.extend(modules::check(root, &selection.modules)?);

            match cargo_lock::read(root)? {
                Ok(lock) => {
                    problems.extend(layer_use::check(
                        root,
                        selection,
                        catalog,
                        lock.as_ref(),
                        package_value,
                    )?);
                    problems.extend(package_boundaries::check(root, selection, lock.as_ref())?);
                }
                Err(problem) => problems.push(problem),
            }
        } else {
            problems.extend(layer_use::check(
                root,
                selection,
                catalog,
                None,
                package_value,
            )?);
        }

        problems.extend(roles::check(root, &selection.roles));
        problems.extend(layout::check(root, selection, catalog)?);
        problems.extend(workflow_pins::check(root, &selection.version)?);
        problems = violations::apply(problems, &selection.violations);
        problems.extend(violations::growth(root, &selection.violations, target));
        problems.extend(weakened::check(root, target));
    }

    Ok(without_duplicates(problems))
}

// Several checks report the same broken manifest, so only the first report of a problem stays.
fn without_duplicates(problems: Vec<Problem>) -> Vec<Problem> {
    let mut unique: Vec<Problem> = Vec::new();

    for problem in problems {
        if unique.contains(&problem) {
            continue;
        }

        unique.push(problem);
    }

    unique
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
            "is missing; add one with `schema = 1`, `version`, and a `language`".to_string(),
        ),
        Installation::Broken(reason) => (Rule::BrokenInstallation, reason.to_string()),
    };

    Some(Problem {
        path: CONFIG_FILE.to_string(),
        rule,
        message,
        key: None,
    })
}

#[cfg(test)]
mod tests;
