use std::fs;

use super::*;
use crate::config::{self, Installation};
use crate::layer;

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

#[test]
fn run_fails_before_writing_for_an_invalid_layer() {
    let cases = [
        (Language::Rust, "nope"),
        (Language::TypeScript, "gpui"),
        (Language::Rust, "react"),
    ];

    for (language, layer) in cases {
        let root = empty_folder();
        let request = request(language, &[layer], &[]);

        let error = run(root.path(), layer::catalog(), &request).expect_err("the layer is invalid");

        let named = match &error {
            InitError::Selection(SelectionError::UnknownLayer { layer }) => layer,
            InitError::Selection(SelectionError::WrongLanguage {
                layer,
                layer_language,
                ..
            }) if *layer_language != language => layer,
            other => panic!("unexpected error {other:?}"),
        };
        assert_eq!(named, layer);
        assert_eq!(entries(root.path()), Vec::<String>::new());
    }
}

#[test]
fn run_fails_before_writing_for_an_unknown_role() {
    let cases = [(Language::Rust, "nope"), (Language::TypeScript, "core")];

    for (language, role) in cases {
        let root = empty_folder();
        let request = request(language, &[], &[role]);

        let error = run(root.path(), layer::catalog(), &request).expect_err("the role is unknown");

        assert!(
            matches!(
                &error,
                InitError::UnknownRole { role: named, language: named_language }
                    if named == role && *named_language == language
            ),
            "{error:?}"
        );
        assert_eq!(entries(root.path()), Vec::<String>::new());
    }
}

#[test]
fn run_with_sources_fails_before_writing_for_an_unknown_role() {
    let root = empty_folder();
    let request = request(Language::TypeScript, &[], &["core"]);

    let error = run_with_sources(root.path(), layer::catalog(), &request, &[])
        .expect_err("the role is unknown");

    assert!(
        matches!(&error, InitError::UnknownRole { role, .. } if role == "core"),
        "{error:?}"
    );
    assert_eq!(entries(root.path()), Vec::<String>::new());
}

#[test]
fn run_fails_before_writing_for_a_duplicate_role() {
    let root = empty_folder();
    let request = request(Language::Rust, &[], &["core", "cli", "core"]);

    let error = run(root.path(), layer::catalog(), &request).expect_err("the role repeats");

    assert!(
        matches!(&error, InitError::DuplicateRole { role } if role == "core"),
        "{error:?}"
    );
    assert_eq!(entries(root.path()), Vec::<String>::new());
}

#[test]
fn run_fails_before_writing_for_a_rust_xtask_name() {
    let root = empty_folder();
    let request = Request {
        name: "xtask".to_string(),
        ..request(Language::Rust, &[], &[])
    };

    let error = run(root.path(), layer::catalog(), &request).expect_err("the name collides");

    assert!(
        matches!(&error, InitError::NameCollision { name } if name == "xtask"),
        "{error:?}"
    );
    assert_eq!(entries(root.path()), Vec::<String>::new());
}

#[test]
fn parse_arguments_rejects_a_name_over_the_limit() {
    let name = "a".repeat(215);

    for language in ["rust", "typescript"] {
        let arguments = arguments(&["--name", name.as_str(), "--language", language]);

        let error = parse_arguments(&arguments).expect_err("the name is too long");

        assert!(
            matches!(&error, InitError::NameTooLong),
            "{language}: {error:?}"
        );
    }
}

#[test]
fn parse_arguments_accepts_a_name_at_the_limit() {
    let name = "a".repeat(214);

    for language in ["rust", "typescript"] {
        let root = empty_folder();
        let arguments = arguments(&["--name", name.as_str(), "--language", language]);

        let request =
            parse_arguments(&arguments).unwrap_or_else(|error| panic!("{language}: {error:?}"));

        run(root.path(), layer::catalog(), &request)
            .unwrap_or_else(|error| panic!("{language}: {error:?}"));

        assert!(root.path().join("stomme.toml").is_file(), "{language}");
    }
}

#[test]
fn run_accepts_a_typescript_xtask_name() {
    let root = empty_folder();
    let request = Request {
        name: "xtask".to_string(),
        ..request(Language::TypeScript, &[], &[])
    };

    run(root.path(), layer::catalog(), &request).expect("typescript accepts the name");

    assert!(root.path().join("stomme.toml").is_file());
}

#[test]
fn run_fails_when_the_folder_is_not_empty() {
    let root = empty_folder();
    let request = request(Language::Rust, &[], &[]);

    fs::write(root.path().join("notes.txt"), "keep\n").expect("write a file");

    let error = run(root.path(), layer::catalog(), &request).expect_err("the folder has a file");

    assert!(
        matches!(&error, InitError::NotEmpty { path } if *path == root.path().display().to_string()),
        "{error:?}"
    );
    assert_eq!(entries(root.path()), vec!["notes.txt".to_string()]);
}

#[test]
fn run_allows_an_existing_git_folder() {
    let root = empty_folder();
    let request = request(Language::Rust, &[], &[]);

    fs::create_dir(root.path().join(".git")).expect("create .git");

    run(root.path(), layer::catalog(), &request).expect("init accepts a .git folder");

    assert!(root.path().join("stomme.toml").is_file());
}

#[test]
fn run_writes_a_synced_project() {
    let root = empty_folder();
    let request = request(Language::Rust, &[], &[]);

    run(root.path(), layer::catalog(), &request).expect("init succeeds");

    assert!(root.path().join("stomme.toml").is_file());
    assert!(root.path().join("docs/stomme/shared/coding.md").is_file());
    assert!(
        root.path()
            .join("docs/stomme/languages/rust/guide.md")
            .is_file()
    );
    assert!(root.path().join(crate::sync::RECORD_FILE).is_file());
    assert!(matches!(
        config::read(root.path(), layer::catalog()),
        Installation::Valid(_)
    ));
}

#[test]
fn parse_arguments_accepts_any_order() {
    let arguments = arguments(&[
        "--roles",
        "core,cli",
        "--layers",
        "",
        "--language",
        "rust",
        "--name",
        "demo-2",
    ]);

    let parsed = parse_arguments(&arguments).expect("the arguments are valid");

    let expected = Request {
        name: "demo-2".to_string(),
        language: Language::Rust,
        layers: Vec::new(),
        roles: vec!["core".to_string(), "cli".to_string()],
    };
    assert_eq!(parsed, expected);
}

#[test]
fn parse_arguments_splits_layers_and_defaults_to_none() {
    let with_layers = arguments(&[
        "--name",
        "demo",
        "--language",
        "typescript",
        "--layers",
        "react,pi",
    ]);
    let without_lists = arguments(&["--name", "demo", "--language", "typescript"]);

    let with_layers = parse_arguments(&with_layers).expect("the arguments are valid");
    let without_lists = parse_arguments(&without_lists).expect("the arguments are valid");

    assert_eq!(
        with_layers,
        request(Language::TypeScript, &["react", "pi"], &[])
    );
    assert_eq!(without_lists, request(Language::TypeScript, &[], &[]));
}

#[test]
fn parse_arguments_rejects_an_unknown_flag() {
    let arguments = arguments(&["--name", "demo", "--language", "rust", "--kind", "app"]);

    let error = parse_arguments(&arguments).expect_err("the flag is unknown");

    assert!(
        matches!(&error, InitError::UnknownFlag { flag } if flag == "--kind"),
        "{error:?}"
    );
}

#[test]
fn parse_arguments_rejects_a_repeated_flag() {
    let arguments = arguments(&["--name", "demo", "--language", "rust", "--name", "other"]);

    let error = parse_arguments(&arguments).expect_err("the flag repeats");

    assert!(
        matches!(&error, InitError::RepeatedFlag { flag } if flag == "--name"),
        "{error:?}"
    );
}

#[test]
fn parse_arguments_rejects_a_missing_value() {
    let arguments = arguments(&["--name", "demo", "--language"]);

    let error = parse_arguments(&arguments).expect_err("the value is missing");

    assert!(
        matches!(&error, InitError::MissingValue { flag } if flag == "--language"),
        "{error:?}"
    );
}

#[test]
fn parse_arguments_rejects_a_missing_flag() {
    let cases = [
        (arguments(&["--language", "rust"]), "--name"),
        (arguments(&["--name", "demo"]), "--language"),
    ];

    for (arguments, missing) in cases {
        let error = parse_arguments(&arguments).expect_err("a flag is missing");

        assert!(
            matches!(&error, InitError::MissingFlag { flag } if *flag == missing),
            "{error:?}"
        );
    }
}

#[test]
fn parse_arguments_rejects_a_bad_language() {
    let arguments = arguments(&["--name", "demo", "--language", "go"]);

    let error = parse_arguments(&arguments).expect_err("the language is unknown");

    assert!(
        matches!(&error, InitError::UnknownLanguage { language } if language == "go"),
        "{error:?}"
    );
}

#[test]
fn parse_arguments_rejects_a_bad_name() {
    for name in ["", "Demo", "1demo", "-demo", "de_mo", "demo.app", "démo"] {
        let arguments = arguments(&["--name", name, "--language", "rust"]);

        let error = parse_arguments(&arguments).expect_err("the name is invalid");

        assert!(
            matches!(&error, InitError::Name { name: named } if named == name),
            "{name:?}: {error:?}"
        );
    }
}

#[test]
fn run_pins_the_style_package() {
    let root = empty_folder();
    let request = request(Language::TypeScript, &[], &[]);

    run(root.path(), layer::catalog(), &request).expect("init succeeds");

    let text = fs::read_to_string(root.path().join("package.json")).expect("read package.json");
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("package.json is JSON");
    let package = "@sqve/stomme-ts-style";

    assert_eq!(
        manifest["devDependencies"][package].as_str(),
        Some(crate::packages::pins()[package].as_str())
    );
}

#[test]
fn plan_rejects_a_typescript_role() {
    let root = empty_folder();
    let request = request(Language::TypeScript, &[], &["core"]);

    let error = run(root.path(), layer::catalog(), &request).expect_err("the role is unknown");

    assert!(
        matches!(
            &error,
            InitError::UnknownRole { role, language }
                if role == "core" && *language == Language::TypeScript
        ),
        "{error:?}"
    );
    assert_eq!(entries(root.path()), Vec::<String>::new());
}
