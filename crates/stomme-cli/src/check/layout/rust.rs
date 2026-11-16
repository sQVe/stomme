use std::collections::{BTreeMap, BTreeSet};

use super::source_files::{SourceFile, Tree};
use super::{Problem, catch_all_folders, is_catch_all, is_snake_case, join, problem};
use crate::rule::Rule;

pub(super) fn check(tree: &Tree) -> Vec<Problem> {
    let mut problems = Vec::new();

    for file in &tree.files {
        problems.extend(file_case(file));
        problems.extend(catch_all_file(file));
        problems.extend(test_placement(file));
    }

    problems.extend(catch_all_folders(tree, Rule::RustCatchAllName));
    problems.extend(owner_folder(tree));

    problems
}

fn file_case(file: &SourceFile) -> Option<Problem> {
    if is_snake_case(&file.stem) {
        return None;
    }

    Some(problem(
        file,
        Rule::RustFileCase,
        format!("names the file `{}`; use snake_case", file.name),
    ))
}

fn catch_all_file(file: &SourceFile) -> Option<Problem> {
    if file.name == "mod.rs" {
        return Some(problem(
            file,
            Rule::RustCatchAllName,
            "names the file `mod.rs`; use `foo.rs` beside `foo/`".to_string(),
        ));
    }

    if is_catch_all(&file.stem) {
        return Some(problem(
            file,
            Rule::RustCatchAllName,
            format!(
                "names the file `{}` after a catch-all; name it after its owner",
                file.name
            ),
        ));
    }

    None
}

fn test_placement(file: &SourceFile) -> Option<Problem> {
    if !file.path.split('/').any(|part| part == "src") {
        return None;
    }

    let in_tests = file.path.split('/').any(|part| part == "tests");
    let suffix = file.stem.ends_with("_test") || file.stem.ends_with("_tests");

    if !in_tests && !suffix {
        return None;
    }

    Some(problem(
        file,
        Rule::RustTestPlacement,
        format!(
            "puts the test file `{}` below `src`; put unit tests in a `#[cfg(test)] mod tests` in the owning file",
            file.name
        ),
    ))
}

fn owner_folder(tree: &Tree) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut by_folder: BTreeMap<&str, Vec<&SourceFile>> = BTreeMap::new();

    for file in &tree.files {
        by_folder
            .entry(file.folder.as_str())
            .or_default()
            .push(file);
    }

    for (folder, files) in &by_folder {
        let names: BTreeSet<&str> = files.iter().map(|file| file.name.as_str()).collect();

        for file in files {
            if is_test_file(file) {
                continue;
            }

            let Some((owner, _)) = file.stem.split_once('_') else {
                continue;
            };
            let base = format!("{owner}.rs");

            if !names.contains(base.as_str()) {
                continue;
            }

            if !tree.folders.contains(&join(folder, owner)) {
                continue;
            }

            problems.push(Problem {
                path: join(folder, &base),
                rule: Rule::RustOwnerFolder,
                message: format!(
                    "puts `{}` outside the owner folder `{owner}/`; move it into `{owner}/`",
                    file.name
                ),
                key: Some(file.name.clone()),
            });
        }
    }

    problems
}

fn is_test_file(file: &SourceFile) -> bool {
    file.stem.ends_with("_test")
        || file.stem.ends_with("_tests")
        || file.path.split('/').any(|part| part == "tests")
}

#[cfg(test)]
mod tests;
