use std::path::Path;

use super::Problem;
use super::source_files::{self, SourceFile, Tree};
use crate::config::Selection;
use crate::layer::{Catalog, Language, Refinement};
use crate::rule::Rule;

mod react;
mod rust;
mod typescript;

#[cfg(test)]
mod tests;

const CATCH_ALL: &[&str] = &["shared", "utils", "helpers", "common", "types"];

pub(super) fn check(
    root: &Path,
    selection: &Selection,
    catalog: &Catalog,
) -> anyhow::Result<Vec<Problem>> {
    let tree = source_files::walk(root, selection.language)?;
    let refinements = catalog.refinements(&selection.layers);
    let mut problems = match selection.language {
        Language::Rust => rust::check(&tree),
        Language::TypeScript => typescript::check(&tree, &refinements)?,
    };

    if selection.language == Language::TypeScript
        && refinements.contains(&Refinement::TsxComponents)
    {
        problems.extend(react::check(&tree));
    }

    Ok(problems)
}

pub(super) fn problem(file: &SourceFile, rule: Rule, message: String) -> Problem {
    Problem {
        path: file.path.clone(),
        rule,
        message,
        key: Some(file.name.clone()),
    }
}

pub(super) fn is_catch_all(name: &str) -> bool {
    CATCH_ALL.contains(&name)
}

pub(super) fn catch_all_folders(tree: &Tree, rule: Rule) -> Vec<Problem> {
    tree.source_folders()
        .into_iter()
        .filter_map(|folder| {
            let name = folder.rsplit('/').next()?;

            if !is_catch_all(name) {
                return None;
            }

            Some(Problem {
                path: folder.to_string(),
                rule,
                message: format!(
                    "names the folder `{name}` after a catch-all; name it after its owner"
                ),
                key: Some(name.to_string()),
            })
        })
        .collect()
}

pub(super) fn is_snake_case(stem: &str) -> bool {
    let mut characters = stem.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    if !first.is_ascii_lowercase() {
        return false;
    }

    let mut after_underscore = false;

    for character in characters {
        if character == '_' {
            if after_underscore {
                return false;
            }

            after_underscore = true;
        } else if character.is_ascii_lowercase() || character.is_ascii_digit() {
            after_underscore = false;
        } else {
            return false;
        }
    }

    !after_underscore
}

pub(super) fn is_camel_case(stem: &str) -> bool {
    let mut characters = stem.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    first.is_ascii_lowercase() && characters.all(|character| character.is_ascii_alphanumeric())
}

pub(super) fn is_pascal_case(stem: &str) -> bool {
    let mut characters = stem.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    first.is_ascii_uppercase() && characters.all(|character| character.is_ascii_alphanumeric())
}
