use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

#[path = "../src/test_support.rs"]
mod test_support;

use test_support::{commit, git, write_file};

#[test]
fn an_empty_scope_prints_the_empty_sentinel() {
    let folder = repository();
    write_file(folder.path(), "src/file.rs", "one\n");
    commit(folder.path(), "base");
    let head = head_commit(folder.path());

    let output = stomme(folder.path(), &scope_arguments(&head, &head, &head, &head));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "empty\n");
    assert_eq!(
        stderr_lines(&output),
        vec!["stomme: scope: 0 files, 0 added lines, 0 removed lines"]
    );
}

#[test]
fn a_scoped_run_prints_the_base_and_the_status_line() {
    let folder = repository();
    write_file(folder.path(), "src/file.rs", "one\n");
    commit(folder.path(), "base");
    let base = head_commit(folder.path());
    write_file(folder.path(), "src/file.rs", "one\ntwo\n");
    commit(folder.path(), "head");
    let head = head_commit(folder.path());

    let output = stomme(folder.path(), &scope_arguments(&base, &base, &base, &head));

    assert_eq!(output.status.code(), Some(0));

    let base_commit = stdout(&output).trim().to_string();

    assert_eq!(base_commit.len(), 40);
    assert!(
        base_commit
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
    assert_eq!(
        stderr_lines(&output),
        vec!["stomme: scope: 1 files, 1 added lines, 0 removed lines"]
    );
}

#[test]
fn a_missing_reviewed_head_fails_without_a_base() {
    let folder = repository();
    write_file(folder.path(), "src/file.rs", "one\n");
    commit(folder.path(), "base");
    let head = head_commit(folder.path());
    let missing = "0".repeat(40);

    let output = stomme(
        folder.path(),
        &scope_arguments(&missing, &head, &head, &head),
    );

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr_lines(&output),
        vec![format!(
            "stomme: `--reviewed` names `{missing}`, which is not a commit in this clone"
        )]
    );
}

#[test]
fn reordered_flags_fall_through_to_the_usage_message() {
    let folder = repository();
    write_file(folder.path(), "src/file.rs", "one\n");
    commit(folder.path(), "base");
    let head = head_commit(folder.path());
    let arguments = [
        "ocr-scope",
        "--reviewed-base",
        &head,
        "--reviewed",
        &head,
        "--base",
        &head,
        "--head",
        &head,
    ];

    let output = stomme(folder.path(), &arguments);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert!(
        stderr_lines(&output)[0].starts_with("Usage: stomme "),
        "{:?}",
        stderr_lines(&output)
    );
}

#[cfg(unix)]
#[test]
fn a_broken_status_stream_keeps_the_base_and_succeeds() {
    use std::fs::File;
    use std::process::Stdio;

    let folder = repository();
    write_file(folder.path(), "src/file.rs", "one\n");
    commit(folder.path(), "base");
    let base = head_commit(folder.path());
    write_file(folder.path(), "src/file.rs", "one\ntwo\n");
    commit(folder.path(), "head");
    let head = head_commit(folder.path());
    let read_only = File::open(folder.path().join("src/file.rs")).expect("open a read-only file");

    let output = Command::new(env!("CARGO_BIN_EXE_stomme"))
        .args(scope_arguments(&base, &base, &base, &head))
        .current_dir(folder.path())
        .stderr(Stdio::from(read_only))
        .output()
        .expect("run stomme");

    assert_eq!(output.status.code(), Some(0));

    let base_commit = stdout(&output).trim().to_string();

    assert_eq!(base_commit.len(), 40);
    assert!(
        base_commit
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
}

fn scope_arguments<'a>(
    reviewed: &'a str,
    reviewed_base: &'a str,
    base: &'a str,
    head: &'a str,
) -> [&'a str; 9] {
    [
        "ocr-scope",
        "--reviewed",
        reviewed,
        "--reviewed-base",
        reviewed_base,
        "--base",
        base,
        "--head",
        head,
    ]
}

fn repository() -> TempDir {
    let folder = tempfile::Builder::new()
        .prefix("stomme-ocr-scope-cli-")
        .tempdir()
        .expect("create a temporary directory");
    git(folder.path(), &["init", "-q"]);

    folder
}

fn head_commit(folder: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(folder)
        .output()
        .expect("run git");

    String::from_utf8(output.stdout)
        .expect("git prints UTF-8")
        .trim()
        .to_string()
}

fn stomme(folder: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stomme"))
        .args(arguments)
        .current_dir(folder)
        .output()
        .expect("run stomme")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_string)
        .collect()
}
