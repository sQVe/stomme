use std::fs;
use std::path::Path;

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
    include_str!("../../../languages/rust/lints.toml"),
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
