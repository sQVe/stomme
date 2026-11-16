use std::fs;
use std::path::Path;
use std::process::Command;

use super::*;

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("run git");

    assert!(output.status.success(), "{arguments:?}: {output:?}");
}

fn repository(files: &[(&str, &str)], publish: bool) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");

    git(root.path(), &["init", "-q"]);

    for (path, text) in files {
        fs::write(root.path().join(path), text).expect("write a fixture file");
    }

    git(root.path(), &["add", "-A"]);
    git(
        root.path(),
        &[
            "-c",
            "user.name=Stomme Test",
            "-c",
            "user.email=stomme@example.com",
            "commit",
            "-qm",
            "initial",
        ],
    );

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
