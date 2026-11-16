use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use super::sync_project;

const ADR_INDEX: &str = "\
# Architecture decision records

Follow the template for every new ADR.

## Index

### Documentation

- [0001: Documentation scope](./0001-documentation-scope.md)

## Retired

| ADR | Title | Replaced by | Last commit |
| --- | ----- | ----------- | ----------- |
| 0002 | Old choice | 0001 | abc1234 |
";

const CARGO_MANIFEST: &str = concat!(
    "[workspace]\nmembers = [\"crates/*\"]\n\n",
    include_str!("../../../foundation/languages/rust/lints.toml"),
);

const MEMBER_MANIFEST: &str = "\
[package]
name = \"cli\"
version = \"0.0.0\"

[lints]
workspace = true
";

const FILES: &[(&str, &str)] = &[
    ("Cargo.toml", CARGO_MANIFEST),
    ("crates/cli/Cargo.toml", MEMBER_MANIFEST),
    (
        "stomme.toml",
        concat!(
            "schema = 1\nversion = \"",
            env!("CARGO_PKG_VERSION"),
            "\"\nlanguage = \"rust\"\n\n[roles]\ncli = \"crates/cli\"\n"
        ),
    ),
    ("AGENTS.md", "# Fixture\n"),
    ("LICENSE", "MIT License\n"),
    ("renovate.json", "{}\n"),
    (
        ".pi/tau.json",
        r#"{ "fullCheck": { "argv": ["cargo", "xtask", "check"] } }"#,
    ),
    ("docs/README.md", "# Documentation\n"),
    ("docs/development.md", "# Development\n"),
    ("docs/adr/README.md", ADR_INDEX),
    ("docs/adr/TEMPLATE.md", "# ADR NNNN: Title\n"),
    (
        "docs/adr/0001-documentation-scope.md",
        "# ADR 0001: Documentation scope\n\n**Status**: Accepted\n",
    ),
];

pub(crate) fn complete_project() -> TempDir {
    let root = write_project();

    sync_project(root.path());

    root
}

fn write_project() -> TempDir {
    let root = tempfile::Builder::new()
        .prefix("stomme-test-")
        .tempdir()
        .expect("create a temporary directory");

    for (path, text) in FILES {
        write_file(root.path(), path, text);
    }

    root
}

pub(crate) fn write_file(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    let parent = path.parent().expect("a fixture file lies inside the root");

    fs::create_dir_all(parent).expect("create the fixture directory");
    fs::write(&path, text).expect("write the fixture file");
}

// Shared fixtures name version 0.0.0 so that a release bump changes no fixture.
pub(crate) fn with_crate_version(config: &str) -> String {
    let version = env!("CARGO_PKG_VERSION");

    config.replacen(
        "version = \"0.0.0\"",
        &format!("version = \"{version}\""),
        1,
    )
}

pub(crate) fn copy_tree(source: &Path, target: &Path) {
    for entry in fs::read_dir(source).expect("read the fixture folder") {
        let entry = entry.expect("read a fixture entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());

        if entry.file_type().expect("read the file type").is_dir() {
            fs::create_dir_all(&destination).expect("create the fixture folder");
            copy_tree(&path, &destination);
        } else {
            fs::copy(&path, &destination).expect("copy the fixture file");
        }
    }
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
            "commit",
            "-qm",
            message,
        ],
    );
}
