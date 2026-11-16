use std::path::Path;

use super::*;
use crate::check::layout::source_files;
use crate::layer::Language;
use crate::rule::Rule;
use crate::test_support::write_file;

fn tree(root: &Path) -> Tree {
    source_files::walk(root, Language::Rust).expect("walk the project")
}

fn assert_problem(problems: &[Problem], path: &str, rule: Rule, key: &str, case: &str) {
    assert_eq!(problems.len(), 1, "{case}: {problems:?}");
    assert_eq!(problems[0].path, path, "{case}");
    assert_eq!(problems[0].rule, rule, "{case}");
    assert_eq!(problems[0].key.as_deref(), Some(key), "{case}");
}

#[test]
fn check_reports_each_rust_layout_break() {
    let cases = [
        (
            "wrong file case",
            &["src/BadName.rs"][..],
            "src/BadName.rs",
            Rule::RustFileCase,
            "BadName.rs",
        ),
        (
            "catch-all file",
            &["src/utils.rs"][..],
            "src/utils.rs",
            Rule::RustCatchAllName,
            "utils.rs",
        ),
        (
            "mod file",
            &["src/report/mod.rs"][..],
            "src/report/mod.rs",
            Rule::RustCatchAllName,
            "mod.rs",
        ),
        (
            "second production file outside the owner folder",
            &["src/report.rs", "src/report/keep.rs", "src/report_extra.rs"][..],
            "src/report.rs",
            Rule::RustOwnerFolder,
            "report_extra.rs",
        ),
        (
            "misplaced test",
            &["src/report_test.rs"][..],
            "src/report_test.rs",
            Rule::RustTestPlacement,
            "report_test.rs",
        ),
    ];

    for (case, files, path, rule, key) in cases {
        let root = tempfile::tempdir().expect("create a temporary directory");

        for file in files {
            write_file(root.path(), file, "");
        }

        let problems = check(&tree(root.path()));

        assert_problem(&problems, path, rule, key, case);
    }
}

#[test]
fn owner_folder_reports_each_family_file() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    for file in [
        "src/report.rs",
        "src/report/keep.rs",
        "src/report_one.rs",
        "src/report_two.rs",
    ] {
        write_file(root.path(), file, "");
    }

    let problems = check(&tree(root.path()));
    let mut keys: Vec<&str> = problems
        .iter()
        .filter(|problem| problem.rule == Rule::RustOwnerFolder)
        .filter_map(|problem| problem.key.as_deref())
        .collect();

    keys.sort();

    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!(keys, ["report_one.rs", "report_two.rs"]);

    for problem in &problems {
        assert_eq!(problem.path, "src/report.rs");
    }
}
