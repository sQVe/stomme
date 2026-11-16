use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use super::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn with_version(fields: &str) -> String {
    format!("schema = 1\nversion = \"{VERSION}\"\n{fields}")
}

fn installation(text: &str) -> Installation {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::write(root.path().join(CONFIG_FILE), text).expect("write stomme.toml");

    read(root.path(), layer::catalog())
}

#[test]
fn read_reports_a_toml_syntax_error_as_broken() {
    assert!(matches!(
        installation(&with_version("language = \"rust\n")),
        Installation::Broken(_)
    ));
}

#[test]
fn read_reports_a_layer_of_another_language_as_broken() {
    let installation = installation(&with_version(
        "language = \"typescript\"\nlayers = [\"gpui\"]\n",
    ));

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("`gpui`"), "{reason}");
    assert!(reason.contains("rust"), "{reason}");
}

#[test]
fn read_reports_a_missing_file_as_no_stomme() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    assert_eq!(read(root.path(), layer::catalog()), Installation::NoStomme);
}

#[test]
fn read_accepts_a_schema_one_file() {
    let text =
        with_version("language = \"rust\"\nlayers = [\"gpui\"]\n\n[roles]\ncli = \"crates/cli\"\n");

    assert_eq!(
        installation(&text),
        Installation::Valid(Selection {
            version: VERSION.to_string(),
            language: Language::Rust,
            layers: BTreeSet::from(["gpui".to_string()]),
            roles: BTreeMap::from([("cli".to_string(), "crates/cli".to_string())]),
        })
    );
}

#[test]
fn read_defaults_to_no_layers_and_no_roles() {
    assert_eq!(
        installation(&with_version("language = \"typescript\"\n")),
        Installation::Valid(Selection {
            version: VERSION.to_string(),
            language: Language::TypeScript,
            layers: BTreeSet::new(),
            roles: BTreeMap::new(),
        })
    );
}

#[test]
fn read_gives_the_same_selection_for_layers_in_either_order() {
    let forward = installation(&with_version(
        "language = \"rust\"\nlayers = [\"gpui\", \"gpui-component\"]\n",
    ));
    let backward = installation(&with_version(
        "language = \"rust\"\nlayers = [\"gpui-component\", \"gpui\"]\n",
    ));

    assert!(matches!(forward, Installation::Valid(_)), "{forward:?}");
    assert_eq!(forward, backward);
}

#[test]
fn read_reports_a_leftover_kind_with_the_fields_that_replace_it() {
    let installation = installation(&with_version("kind = \"rust-cli\"\n"));

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    for field in ["`language`", "`layers`", "`[roles]`"] {
        assert!(reason.contains(field), "{field}: {reason}");
    }
}

#[test]
fn read_reports_a_bad_layer_set_naming_the_layer() {
    let cases = [
        (
            "unknown layer",
            "language = \"rust\"\nlayers = [\"egui\"]\n",
            "`egui`",
        ),
        (
            "duplicate layer",
            "language = \"rust\"\nlayers = [\"gpui\", \"gpui\"]\n",
            "`gpui`",
        ),
        (
            "missing required layer",
            "language = \"rust\"\nlayers = [\"gpui-component\"]\n",
            "`gpui`",
        ),
    ];

    for (case, text, layer) in cases {
        let installation = installation(&with_version(text));

        let Installation::Broken(reason) = installation else {
            panic!("{case}: expected a broken installation, got {installation:?}");
        };
        assert!(reason.contains(layer), "{case}: {reason}");
    }
}

#[test]
fn read_reports_a_version_other_than_the_binary_version_as_broken() {
    let installation = installation("schema = 1\nversion = \"9.9.9\"\nlanguage = \"rust\"\n");

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("9.9.9"), "{reason}");
    assert!(reason.contains(VERSION), "{reason}");
}

#[test]
fn read_reports_a_file_that_does_not_match_schema_one_as_broken() {
    let cases = [
        (
            "unknown field",
            with_version("language = \"rust\"\nname = \"x\"\n"),
        ),
        (
            "missing version",
            "schema = 1\nlanguage = \"rust\"\n".to_string(),
        ),
        ("missing language", with_version("")),
        ("unknown language", with_version("language = \"go\"\n")),
        (
            "layers not a list",
            with_version("language = \"rust\"\nlayers = \"gpui\"\n"),
        ),
        (
            "role not a path",
            with_version("language = \"rust\"\n[roles]\ncli = 1\n"),
        ),
        (
            "schema 2",
            format!("schema = 2\nversion = \"{VERSION}\"\nlanguage = \"rust\"\n"),
        ),
        (
            "missing schema",
            format!("version = \"{VERSION}\"\nlanguage = \"rust\"\n"),
        ),
        (
            "text schema",
            format!("schema = \"1\"\nversion = \"{VERSION}\"\nlanguage = \"rust\"\n"),
        ),
    ];

    for (case, text) in cases {
        assert!(
            matches!(installation(&text), Installation::Broken(_)),
            "{case}"
        );
    }
}

#[test]
fn read_reports_a_file_it_cannot_read_as_broken() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::create_dir(root.path().join(CONFIG_FILE)).expect("create a stomme.toml directory");

    assert!(matches!(
        read(root.path(), layer::catalog()),
        Installation::Broken(_)
    ));
}
