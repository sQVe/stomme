use std::fs;

use std::path::Path;

use super::*;
use crate::check::cargo_manifest::read_root;
use crate::layer;
use crate::test_project::complete_project;

fn check_project(root: &Path) -> Vec<Problem> {
    let manifest = read_root(root)
        .expect("read Cargo.toml")
        .expect("Cargo.toml is valid");

    check(&manifest, layer::catalog().rust_lints())
}

fn replace_in_manifest(root: &Path, from: &str, to: &str) {
    let path = root.join("Cargo.toml");
    let manifest = fs::read_to_string(&path).expect("read Cargo.toml");

    assert!(manifest.contains(from), "Cargo.toml has {from:?}");
    fs::write(&path, manifest.replacen(from, to, 1)).expect("write Cargo.toml");
}

fn assert_single_problem(problems: &[Problem], path: &str, rule: Rule, named: &str, case: &str) {
    assert_eq!(problems.len(), 1, "{case}: {problems:?}");
    assert_eq!(problems[0].path, path, "{case}");
    assert_eq!(problems[0].rule, rule, "{case}");
    assert!(
        problems[0].message.contains(named),
        "{case}: {}",
        problems[0].message
    );
}

#[test]
fn check_passes_a_complete_project() {
    let root = complete_project();

    assert_eq!(check_project(root.path()), []);
}

#[test]
fn check_reports_a_shared_lint_set_to_a_lower_level() {
    let root = complete_project();

    replace_in_manifest(root.path(), "todo = \"warn\"", "todo = \"allow\"");

    let problems = check_project(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, "Cargo.toml");
    assert_eq!(problems[0].rule, Rule::RustLintLevel);
    assert!(
        problems[0].message.contains("`clippy::todo`"),
        "{}",
        problems[0].message
    );
}

#[test]
fn check_reports_a_weakened_shared_lint_naming_it() {
    let cases = [
        (
            "missing clippy lint",
            "dbg_macro = \"warn\"\n",
            "",
            "`clippy::dbg_macro`",
        ),
        (
            "missing rust lint",
            "unsafe_code = \"deny\"\n",
            "",
            "`unsafe_code`",
        ),
        (
            "lower rust level",
            "unsafe_code = \"deny\"",
            "unsafe_code = { level = \"warn\" }",
            "`unsafe_code`",
        ),
        (
            "other priority",
            "all = { level = \"warn\", priority = -2 }",
            "all = { level = \"warn\", priority = -3 }",
            "`clippy::all`",
        ),
        (
            "missing priority",
            "all = { level = \"warn\", priority = -2 }",
            "all = \"warn\"",
            "`clippy::all`",
        ),
    ];

    for (case, from, to, named) in cases {
        let root = complete_project();

        replace_in_manifest(root.path(), from, to);

        let problems = check_project(root.path());

        assert_single_problem(&problems, "Cargo.toml", Rule::RustLintLevel, named, case);
    }
}

#[test]
fn check_passes_added_lints_and_raised_levels() {
    let root = complete_project();

    replace_in_manifest(root.path(), "todo = \"warn\"", "todo = \"deny\"");
    replace_in_manifest(
        root.path(),
        "unsafe_code = \"deny\"",
        "unsafe_code = { level = \"forbid\", priority = 0 }",
    );
    replace_in_manifest(
        root.path(),
        "dbg_macro = \"warn\"",
        "dbg_macro = { level = \"warn\" }\nunwrap_used = \"warn\"",
    );

    assert_eq!(check_project(root.path()), []);
}

#[test]
fn check_names_the_required_level_and_priority_of_a_missing_lint() {
    let root = complete_project();

    replace_in_manifest(
        root.path(),
        "all = { level = \"warn\", priority = -2 }\n",
        "",
    );

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "Cargo.toml",
        Rule::RustLintLevel,
        "`warn` with priority -2",
        "missing clippy::all",
    );
}
