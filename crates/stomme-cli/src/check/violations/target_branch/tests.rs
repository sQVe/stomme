use std::ffi::OsString;
use std::fs;
use std::path::Path;

use super::*;
use crate::test_support::{commit, git};

fn repository(files: &[(&str, &str)], publish: bool) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");

    git(root.path(), &["init", "-q"]);

    for (path, text) in files {
        fs::write(root.path().join(path), text).expect("write a fixture file");
    }

    commit(root.path(), "initial");

    if publish {
        git(
            root.path(),
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
    }

    root
}

fn read(root: &Path) -> TargetRead {
    let target = TargetBranch::git("origin/main".to_string());

    (target.read)(root, &target.reference)
}

#[test]
fn git_reader_reads_each_target_state() {
    let with_config = repository(&[("stomme.toml", "schema = 1\n")], true);
    let without_config = repository(&[("README.md", "# Readme\n")], true);
    let missing_ref = repository(&[("README.md", "# Readme\n")], false);

    assert_eq!(
        read(with_config.path()),
        TargetRead::Config("schema = 1\n".to_string())
    );
    assert_eq!(read(without_config.path()), TargetRead::NoConfig);
    assert!(matches!(
        read(missing_ref.path()),
        TargetRead::Unavailable(_)
    ));
}

#[test]
fn default_reference_maps_the_pull_request_base() {
    let without_pull_request = default_reference(None).expect("no pull request base");
    let empty = default_reference(Some(OsString::from(""))).expect("an empty base");
    let named = default_reference(Some(OsString::from("stack/parent"))).expect("a base name");

    assert_eq!(without_pull_request, "origin/main");
    assert_eq!(empty, "origin/main");
    assert_eq!(named, "origin/stack/parent");
}

#[cfg(unix)]
#[test]
fn default_reference_rejects_a_non_utf8_pull_request_base() {
    use std::os::unix::ffi::OsStringExt as _;

    let base = OsString::from_vec(vec![0xff]);
    let error = default_reference(Some(base)).expect_err("a non-UTF-8 base");

    assert!(error.to_string().contains("GITHUB_BASE_REF"), "{error:#}");
}
