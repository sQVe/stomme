use std::fs;
use std::path::Path;

use super::*;
use crate::config::{self, Installation};
use crate::layer;
use crate::rule::Rule;
use crate::test_support::{complete_project, write_file};

fn selection(root: &Path) -> Selection {
    match config::read(root, layer::catalog()) {
        Installation::Valid(selection) => selection,
        other => panic!("expected a valid installation, got {other:?}"),
    }
}

#[test]
fn check_reports_an_edited_synced_file() {
    let root = complete_project();
    let path = "docs/stomme/shared/coding.md";

    write_file(root.path(), path, "edited\n");

    let problems = check(root.path(), &selection(root.path())).expect("check the project");

    assert!(
        problems
            .iter()
            .any(|problem| problem.path == path && problem.rule == Rule::SyncedFiles),
        "{problems:?}"
    );
}

fn rewrite_record(root: &Path, change: impl FnOnce(&mut Record)) {
    let mut record = Record::read(root)
        .expect("read the record")
        .expect("the record exists");

    change(&mut record);

    let text = toml::to_string(&record).expect("serialize the record");

    write_file(root, RECORD_FILE, &text);
}

fn reported(problems: &[Problem], path: &str) -> bool {
    problems
        .iter()
        .any(|problem| problem.path == path && problem.rule == Rule::SyncedFiles)
}

#[test]
fn check_reports_a_missing_synced_file() {
    let root = complete_project();
    let path = "docs/stomme/shared/coding.md";

    fs::remove_file(root.path().join(path)).expect("remove the synced file");

    let problems = check(root.path(), &selection(root.path())).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[test]
fn check_reports_an_extra_file_under_docs_stomme() {
    let root = complete_project();
    let path = "docs/stomme/extra.md";

    write_file(root.path(), path, "extra\n");

    let problems = check(root.path(), &selection(root.path())).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[test]
fn check_reports_a_missing_record() {
    let root = complete_project();

    fs::remove_file(root.path().join(RECORD_FILE)).expect("remove the record");

    let problems = check(root.path(), &selection(root.path())).expect("check the project");

    assert!(reported(&problems, RECORD_FILE), "{problems:?}");
}

#[test]
fn check_reports_a_malformed_record() {
    let root = complete_project();

    write_file(root.path(), RECORD_FILE, "not toml\n");

    let problems = check(root.path(), &selection(root.path())).expect("check the project");

    assert!(reported(&problems, RECORD_FILE), "{problems:?}");
}

#[test]
fn check_reports_a_record_version_that_differs() {
    let root = complete_project();

    rewrite_record(root.path(), |record| record.version = "9.9.9".to_string());

    let problems = check(root.path(), &selection(root.path())).expect("check the project");
    let problem = problems
        .iter()
        .find(|problem| problem.path == RECORD_FILE)
        .expect("the record is reported");

    assert!(problem.message.contains("9.9.9"), "{problem:?}");
    assert!(
        problem.message.contains(env!("CARGO_PKG_VERSION")),
        "{problem:?}"
    );
}

#[test]
fn check_reports_a_record_hash_that_differs() {
    let root = complete_project();
    let path = "docs/stomme/shared/coding.md";

    rewrite_record(root.path(), |record| {
        record.files.insert(path.to_string(), "0".repeat(64));
    });

    let problems = check(root.path(), &selection(root.path())).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[test]
fn check_reports_a_recorded_file_that_is_not_owned() {
    let root = complete_project();
    let path = "docs/stomme/old.md";

    rewrite_record(root.path(), |record| {
        record.files.insert(path.to_string(), "0".repeat(64));
    });

    let problems = check(root.path(), &selection(root.path())).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[test]
fn check_reports_a_directory_where_a_synced_file_belongs() {
    let root = complete_project();
    let path = "docs/stomme/shared/coding.md";

    fs::remove_file(root.path().join(path)).expect("remove the synced file");
    fs::create_dir(root.path().join(path)).expect("create a directory in its place");

    let problems = check(root.path(), &selection(root.path())).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}
