use std::fs;

use super::*;

fn installation(text: &str) -> Installation {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::write(root.path().join(CONFIG_FILE), text).expect("write stomme.toml");

    read(root.path())
}

#[test]
fn read_reports_a_toml_syntax_error_as_broken() {
    assert!(matches!(
        installation("schema = 1\nkind = \"rust-cli\n"),
        Installation::Broken(_)
    ));
}

#[test]
fn read_reports_a_missing_file_as_no_stomme() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    assert_eq!(read(root.path()), Installation::NoStomme);
}

#[test]
fn read_accepts_every_kind_of_schema_one() {
    let cases = [
        ("rust-gpui-app", Kind::RustGpuiApp),
        ("rust-cli", Kind::RustCli),
        ("ts-web-app", Kind::TsWebApp),
        ("ts-package", Kind::TsPackage),
    ];

    for (name, kind) in cases {
        assert_eq!(
            installation(&format!("schema = 1\nkind = \"{name}\"\n")),
            Installation::Valid(Config { kind }),
            "{name}"
        );
    }
}

#[test]
fn read_reports_a_file_that_does_not_match_schema_one_as_broken() {
    let cases = [
        (
            "unknown field",
            "schema = 1\nkind = \"rust-cli\"\nname = \"x\"\n",
        ),
        ("missing kind", "schema = 1\n"),
        ("unknown kind", "schema = 1\nkind = \"go-cli\"\n"),
        ("schema 2", "schema = 2\nkind = \"rust-cli\"\n"),
        ("missing schema", "kind = \"rust-cli\"\n"),
        ("text schema", "schema = \"1\"\nkind = \"rust-cli\"\n"),
    ];

    for (case, text) in cases {
        assert!(
            matches!(installation(text), Installation::Broken(_)),
            "{case}"
        );
    }
}

#[test]
fn read_reports_a_file_it_cannot_read_as_broken() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::create_dir(root.path().join(CONFIG_FILE)).expect("create a stomme.toml directory");

    assert!(matches!(read(root.path()), Installation::Broken(_)));
}
