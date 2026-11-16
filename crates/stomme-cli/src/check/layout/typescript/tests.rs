use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::*;
use crate::check::layout::source_files;
use crate::layer::Language;
use crate::rule::Rule;
use crate::test_support::write_file;

fn tree(root: &Path) -> Tree {
    source_files::walk(root, Language::TypeScript).expect("walk the project")
}

fn assert_problem(problems: &[Problem], path: &str, rule: Rule, key: &str, case: &str) {
    assert_eq!(problems.len(), 1, "{case}: {problems:?}");
    assert_eq!(problems[0].path, path, "{case}");
    assert_eq!(problems[0].rule, rule, "{case}");
    assert_eq!(problems[0].key.as_deref(), Some(key), "{case}");
}

#[test]
fn check_reports_each_typescript_layout_break() {
    let cases = [
        (
            "wrong file case",
            &["src/BadName.ts"][..],
            "src/BadName.ts",
            Rule::TypeScriptFileCase,
            "BadName.ts",
        ),
        (
            "catch-all file",
            &["src/helpers.ts"][..],
            "src/helpers.ts",
            Rule::TypeScriptCatchAllName,
            "helpers.ts",
        ),
        (
            "index file",
            &["src/index.ts"][..],
            "src/index.ts",
            Rule::NoIndexFile,
            "index.ts",
        ),
        (
            "second production file outside the owner folder",
            &["src/report.ts", "src/reportIncomplete.ts"][..],
            "src/report.ts",
            Rule::TypeScriptOwnerFolder,
            "reportIncomplete.ts",
        ),
        (
            "misplaced test",
            &["src/missing.test.ts"][..],
            "src/missing.test.ts",
            Rule::TestFileName,
            "missing.test.ts",
        ),
        (
            "catch-all folder",
            &["src/helpers/report.ts"][..],
            "src/helpers",
            Rule::TypeScriptCatchAllName,
            "helpers",
        ),
        (
            "owner folder beside a family file",
            &["src/report/report.ts", "src/reportIncomplete.ts"][..],
            "src/report",
            Rule::TypeScriptOwnerFolder,
            "reportIncomplete.ts",
        ),
        (
            "nested owner folder beside a family file",
            &["src/a/report/report.ts", "src/a/reportIncomplete.ts"][..],
            "src/a/report",
            Rule::TypeScriptOwnerFolder,
            "reportIncomplete.ts",
        ),
    ];

    for (case, files, path, rule, key) in cases {
        let root = tempfile::tempdir().expect("create a temporary directory");

        for file in files {
            write_file(root.path(), file, "");
        }

        let problems = check(&tree(root.path()), &BTreeSet::new()).expect("check the project");

        assert_problem(&problems, path, rule, key, case);
    }
}

#[test]
fn check_passes_pascal_case_class_files() {
    let cases = [
        ("named export", "export class Report {}"),
        ("default export", "export default class Report {}"),
    ];

    for (case, text) in cases {
        let root = tempfile::tempdir().expect("create a temporary directory");

        write_file(root.path(), "src/Report.ts", text);

        let problems = check(&tree(root.path()), &BTreeSet::new()).expect("check the project");

        assert_eq!(problems, Vec::<Problem>::new(), "{case}");
    }
}

#[test]
fn check_reports_a_pascal_case_file_it_cannot_read() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), "src/Report.ts", "");
    fs::write(root.path().join("src/Report.ts"), [0xff, 0xfe]).expect("write invalid UTF-8");

    let error = check(&tree(root.path()), &BTreeSet::new()).expect_err("report the read error");

    assert!(
        error.to_string().contains("Could not read src/Report.ts"),
        "{error}"
    );
}

#[test]
fn check_passes_a_test_named_after_its_owner_family() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), "src/report.ts", "");
    write_file(root.path(), "src/reportIncomplete.test.ts", "");

    let problems = check(&tree(root.path()), &BTreeSet::new()).expect("check the project");

    assert_eq!(problems, Vec::<Problem>::new());
}

#[test]
fn check_passes_a_test_named_after_a_pascal_case_class_owner() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), "src/Report.ts", "export class Report {}");
    write_file(root.path(), "src/Report.test.ts", "");

    let problems = check(&tree(root.path()), &BTreeSet::new()).expect("check the project");

    assert_eq!(problems, Vec::<Problem>::new());
}

#[test]
fn check_passes_a_pascal_case_test_named_after_a_class_owner_family() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), "src/Report.ts", "export class Report {}");
    write_file(root.path(), "src/ReportIncomplete.test.ts", "");

    let problems = check(&tree(root.path()), &BTreeSet::new()).expect("check the project");

    assert_eq!(problems, Vec::<Problem>::new());
}

#[test]
fn check_reports_a_class_test_family_when_the_owner_exports_no_class() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), "src/Report.ts", "");
    write_file(root.path(), "src/ReportIncomplete.test.ts", "");

    let problems = check(&tree(root.path()), &BTreeSet::new()).expect("check the project");
    let test_file_problems: Vec<&Problem> = problems
        .iter()
        .filter(|problem| problem.path == "src/ReportIncomplete.test.ts")
        .collect();

    assert_eq!(test_file_problems.len(), 1, "{problems:?}");
    assert_eq!(test_file_problems[0].rule, Rule::TypeScriptFileCase);
}
