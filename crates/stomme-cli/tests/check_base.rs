use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../src/test_support.rs"]
#[expect(
    dead_code,
    reason = "check_base uses a subset of the shared test helpers"
)]
mod test_support;

use test_support::{commit, complete_project, git, write_file};

fn sync_project(root: &Path) {
    let output = stomme(root, &["sync"], None);

    assert_eq!(output.status.code(), Some(0), "sync: {output:?}");
}

fn project() -> tempfile::TempDir {
    let root = complete_project();
    let version = env!("CARGO_PKG_VERSION");
    let without_entry = format!(
        "schema = 1\nversion = \"{version}\"\nlanguage = \"rust\"\n\n[roles]\ncli = \"src/domain\"\n"
    );
    let with_entry = format!(
        "{without_entry}\n[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\nkey = \"cli:domain\"\n"
    );

    fs::create_dir_all(root.path().join("src/domain")).expect("create the role folder");
    write_file(root.path(), "stomme.toml", &without_entry);
    git(root.path(), &["init", "-q"]);
    commit(root.path(), "without the entry");
    git(
        root.path(),
        &["update-ref", "refs/remotes/origin/parent", "HEAD"],
    );
    write_file(root.path(), "stomme.toml", &with_entry);
    commit(root.path(), "with the entry");
    git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );

    root
}

fn stomme(root: &Path, arguments: &[&str], pull_request_base: Option<&OsStr>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_stomme"));

    command
        .args(arguments)
        .current_dir(root)
        .env_remove("GITHUB_BASE_REF");

    if let Some(base) = pull_request_base {
        command.env("GITHUB_BASE_REF", base);
    }

    command.output().expect("run stomme")
}

fn assert_growth_names_the_added_entry(output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr.contains("[violation-growth]"), "{output:?}");
    assert!(
        stderr.contains("rule `role-folder` at `src/domain` with key `cli:domain`"),
        "{output:?}"
    );
    assert!(!stderr.contains("fetch"), "{output:?}");
}

#[test]
fn check_compares_a_pull_request_with_its_target_branch() {
    let root = project();

    let output = stomme(root.path(), &["check"], Some(OsStr::new("parent")));

    assert_growth_names_the_added_entry(&output);
}

#[test]
fn check_resolves_the_pull_request_base_as_a_remote_tracking_ref() {
    let root = project();
    git(
        root.path(),
        &["update-ref", "refs/tags/origin/parent", "HEAD"],
    );

    let output = stomme(root.path(), &["check"], Some(OsStr::new("parent")));

    assert_growth_names_the_added_entry(&output);
}

#[test]
fn check_base_flag_wins_over_the_pull_request_base() {
    let root = project();

    let output = stomme(
        root.path(),
        &["check", "--base", "origin/main"],
        Some(OsStr::new("parent")),
    );

    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[test]
fn check_ignores_an_empty_pull_request_base() {
    let root = project();

    let output = stomme(root.path(), &["check"], Some(OsStr::new("")));

    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[test]
fn check_names_an_unfetched_pull_request_base() {
    let root = project();

    let output = stomme(root.path(), &["check"], Some(OsStr::new("missing")));
    let lines = String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].ends_with("[violation-growth]"), "{lines:?}");
    assert!(
        lines[0].contains("fetch `refs/remotes/origin/missing` and try again"),
        "{lines:?}"
    );
}

#[cfg(unix)]
#[test]
fn check_reports_a_non_utf8_pull_request_base() {
    use std::os::unix::ffi::OsStrExt as _;

    let root = project();
    let base = OsStr::from_bytes(&[0xff]);

    let output = stomme(root.path(), &["check"], Some(base));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("GITHUB_BASE_REF is not valid UTF-8"),
        "{output:?}"
    );
}
