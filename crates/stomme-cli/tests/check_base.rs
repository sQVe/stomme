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

fn stomme(root: &Path, arguments: &[&str], pull_request_base: Option<&str>) -> Output {
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

#[test]
fn check_compares_a_pull_request_with_its_target_branch() {
    let root = project();

    let output = stomme(root.path(), &["check"], Some("parent"));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[violation-growth]"),
        "{output:?}"
    );
}

#[test]
fn check_base_flag_wins_over_the_pull_request_base() {
    let root = project();

    let output = stomme(
        root.path(),
        &["check", "--base", "origin/main"],
        Some("parent"),
    );

    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[test]
fn check_ignores_an_empty_pull_request_base() {
    let root = project();

    let output = stomme(root.path(), &["check"], Some(""));

    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[test]
fn check_names_an_unfetched_pull_request_base() {
    let root = project();

    let output = stomme(root.path(), &["check"], Some("missing"));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[violation-growth]"),
        "{output:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("origin/missing"),
        "{output:?}"
    );
}
