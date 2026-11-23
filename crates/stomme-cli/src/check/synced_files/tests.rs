use std::fs;
use std::path::Path;

use super::*;
use crate::config::{self, Installation};
use crate::layer;
use crate::ocr_rules::PROJECT_RULE_FILE;
use crate::rule::Rule;
use crate::test_support::{complete_project, write_file};

fn selection(root: &Path) -> Selection {
    match config::read(root, layer::catalog(), &config::FileRead::filesystem()) {
        Installation::Valid(selection) => selection,
        other => panic!("expected a valid installation, got {other:?}"),
    }
}

#[test]
fn check_reports_an_edited_synced_file() {
    let root = complete_project();
    let path = "docs/stomme/shared/coding.md";

    write_file(root.path(), path, "edited\n");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(
        problems
            .iter()
            .any(|problem| problem.path == path && problem.rule == Rule::SyncedFiles),
        "{problems:?}"
    );
}

#[test]
fn check_reports_a_stale_ocr_rule_file() {
    let root = complete_project();
    let path = "docs/stomme/ocr-rules.json";

    write_file(
        root.path(),
        PROJECT_RULE_FILE,
        r#"{"rules":[{"path":"src/**","rule":"Keep modules small."}]}"#,
    );

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");
    let problem = problems
        .iter()
        .find(|problem| problem.path == path)
        .expect("the rule file is reported");

    assert!(
        problem.message.contains("differs from the synced content"),
        "{problem:?}"
    );
}

#[test]
fn check_reports_an_invalid_project_rule_file() {
    let root = complete_project();

    write_file(root.path(), PROJECT_RULE_FILE, "{ not json");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");
    let problem = problems
        .iter()
        .find(|problem| problem.path == PROJECT_RULE_FILE)
        .expect("the project rule file is reported");

    assert_eq!(problem.rule, Rule::SyncedFiles);
    assert!(problem.key.is_none(), "{problem:?}");

    fs::remove_dir_all(root.path().join("docs/stomme")).expect("remove the synced output");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, PROJECT_RULE_FILE), "{problems:?}");
    assert!(reported(&problems, RECORD_FILE), "{problems:?}");
}

#[test]
fn check_reports_an_invalid_project_rule_file_when_the_output_parent_is_a_file() {
    let root = complete_project();

    write_file(root.path(), PROJECT_RULE_FILE, "{ not json");
    fs::remove_dir_all(root.path().join("docs/stomme")).expect("remove the synced output");
    write_file(root.path(), "docs/stomme", "not a folder\n");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, PROJECT_RULE_FILE), "{problems:?}");
    assert!(reported(&problems, "docs/stomme"), "{problems:?}");
}

#[cfg(unix)]
#[test]
fn check_reports_a_symlinked_shared_folder() {
    use std::os::unix::fs::symlink;

    let root = complete_project();
    let outside = tempfile::tempdir().expect("create an outside directory");

    fs::rename(
        root.path().join("docs/stomme/shared"),
        outside.path().join("shared"),
    )
    .expect("move shared outside the project");
    symlink(
        outside.path().join("shared"),
        root.path().join("docs/stomme/shared"),
    )
    .expect("create the symlink");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, "docs/stomme/shared"), "{problems:?}");
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

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[test]
fn check_reports_an_extra_file_under_docs_stomme() {
    let root = complete_project();
    let path = "docs/stomme/extra.md";

    write_file(root.path(), path, "extra\n");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[test]
fn check_reports_a_missing_record() {
    let root = complete_project();

    fs::remove_file(root.path().join(RECORD_FILE)).expect("remove the record");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, RECORD_FILE), "{problems:?}");
}

#[test]
fn check_reports_a_malformed_record() {
    let root = complete_project();

    write_file(root.path(), RECORD_FILE, "not toml\n");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, RECORD_FILE), "{problems:?}");
}

#[test]
fn check_reports_a_record_version_that_differs() {
    let root = complete_project();

    rewrite_record(root.path(), |record| record.version = "9.9.9".to_string());

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");
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
        let key = owned::record_key(path).expect("a project path");

        record.files.insert(key.to_string(), "0".repeat(64));
    });

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[test]
fn check_reports_a_synced_file_missing_from_the_record() {
    let root = complete_project();
    let path = "docs/stomme/shared/coding.md";

    rewrite_record(root.path(), |record| {
        record
            .files
            .remove(owned::record_key(path).expect("a project path"));
    });

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");
    let problem = problems
        .iter()
        .find(|problem| problem.path == path && problem.rule == Rule::SyncedFiles)
        .expect("the file is reported");

    assert!(
        problem.message.contains("not in the ownership record"),
        "{problem:?}"
    );
}

#[cfg(unix)]
#[test]
fn check_reports_a_non_utf8_entry_name() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let root = complete_project();
    let folder = root
        .path()
        .join("docs/stomme")
        .join(OsStr::from_bytes(b"\xff"));

    fs::create_dir_all(&folder).expect("create a folder with a non-UTF-8 name");
    fs::write(folder.join("extra.md"), "extra\n").expect("write the extra file");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");
    let problem = problems
        .iter()
        .find(|problem| problem.message.contains("UTF-8"))
        .expect("the entry is reported");

    assert!(problem.path.contains("\\xFF"), "{problem:?}");
    assert_eq!(problem.rule, Rule::SyncedFiles);
}

#[test]
fn check_reports_a_recorded_file_that_is_not_owned() {
    let root = complete_project();
    let path = "docs/stomme/old.md";

    rewrite_record(root.path(), |record| {
        let key = owned::record_key(path).expect("a project path");

        record.files.insert(key.to_string(), "0".repeat(64));
    });

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[test]
fn check_reports_a_directory_where_a_synced_file_belongs() {
    let root = complete_project();
    let path = "docs/stomme/shared/coding.md";

    fs::remove_file(root.path().join(path)).expect("remove the synced file");
    fs::create_dir(root.path().join(path)).expect("create a directory in its place");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, path), "{problems:?}");
}

#[cfg(unix)]
#[test]
fn check_reports_a_symlinked_docs_folder() {
    use std::os::unix::fs::symlink;

    let root = complete_project();
    let outside = tempfile::tempdir().expect("create an outside directory");

    fs::rename(root.path().join("docs"), outside.path().join("docs"))
        .expect("move docs outside the project");
    symlink(outside.path().join("docs"), root.path().join("docs")).expect("create the symlink");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, "docs"), "{problems:?}");
}

#[cfg(unix)]
#[test]
fn check_reports_a_symlinked_docs_stomme_folder() {
    use std::os::unix::fs::symlink;

    let root = complete_project();
    let outside = tempfile::tempdir().expect("create an outside directory");

    fs::rename(
        root.path().join("docs/stomme"),
        outside.path().join("stomme"),
    )
    .expect("move docs/stomme outside the project");
    symlink(
        outside.path().join("stomme"),
        root.path().join("docs/stomme"),
    )
    .expect("create the symlink");

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(reported(&problems, "docs/stomme"), "{problems:?}");
}

#[test]
fn check_reports_an_edited_agents_block() {
    let root = complete_project();
    let path = crate::agents_block::PATH;
    let text = fs::read_to_string(root.path().join(path)).expect("read AGENTS.md");
    let range = crate::agents_block::find(&text)
        .expect("find the block")
        .expect("the block exists");
    let edited = format!(
        "{}{}\nedited\n{}\n{}",
        &text[..range.start],
        crate::agents_block::START,
        crate::agents_block::END,
        &text[range.end..]
    );

    write_file(root.path(), path, &edited);

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(
        problems
            .iter()
            .any(|problem| problem.path == path && problem.rule == Rule::AgentsBlock),
        "{problems:?}"
    );
}

#[test]
fn check_reports_a_missing_agents_marker() {
    let root = complete_project();
    let path = crate::agents_block::PATH;
    let text = fs::read_to_string(root.path().join(path)).expect("read AGENTS.md");
    let without_end = text.replace(&format!("{}\n", crate::agents_block::END), "");

    write_file(root.path(), path, &without_end);

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");

    assert!(
        problems
            .iter()
            .any(|problem| problem.path == path && problem.rule == Rule::AgentsBlock),
        "{problems:?}"
    );
}

#[test]
fn check_reports_an_agents_block_missing_from_the_record() {
    let root = complete_project();
    let path = crate::agents_block::PATH;

    rewrite_record(root.path(), |record| {
        record.agents = None;
    });

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");
    let problem = problems
        .iter()
        .find(|problem| problem.path == path && problem.rule == Rule::AgentsBlock)
        .expect("the block is reported");

    assert!(
        problem.message.contains("not in the ownership record"),
        "{problem:?}"
    );
}

#[test]
fn check_reports_an_agents_block_record_hash_that_differs() {
    let root = complete_project();
    let path = crate::agents_block::PATH;

    rewrite_record(root.path(), |record| {
        record.agents = Some("0".repeat(64));
    });

    let problems =
        check(root.path(), &selection(root.path()), layer::catalog()).expect("check the project");
    let problem = problems
        .iter()
        .find(|problem| problem.path == path && problem.rule == Rule::AgentsBlock)
        .expect("the block is reported");

    assert!(
        problem
            .message
            .contains("does not match the ownership record"),
        "{problem:?}"
    );
}
