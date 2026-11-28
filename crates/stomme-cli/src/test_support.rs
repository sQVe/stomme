use std::fs;
use std::path::Path;
use std::process::Command;

pub(crate) fn write_file(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    let parent = path.parent().expect("a fixture file lies inside the root");

    fs::create_dir_all(parent).expect("create the fixture directory");
    fs::write(&path, text).expect("write the fixture file");
}

pub(crate) fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("run git");

    assert!(output.status.success(), "{arguments:?}: {output:?}");
}

pub(crate) fn commit(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(
        root,
        &[
            "-c",
            "user.name=Stomme Test",
            "-c",
            "user.email=stomme@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            message,
        ],
    );
}
