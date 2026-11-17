use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::config::{self, Installation};
use crate::layer;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn request(language: Language, layers: &[&str], roles: &[&str]) -> Request {
    Request {
        name: "demo".to_string(),
        language,
        layers: layers.iter().map(ToString::to_string).collect(),
        roles: roles.iter().map(ToString::to_string).collect(),
    }
}

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

fn empty_folder() -> TempDir {
    tempfile::Builder::new()
        .prefix("stomme-init-")
        .tempdir()
        .expect("create a temporary directory")
}

fn entries(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root)
        .expect("read the target folder")
        .map(|entry| {
            let entry = entry.expect("read a folder entry");

            entry.file_name().to_string_lossy().into_owned()
        })
        .collect();

    names.sort();

    names
}

fn planned(request: &Request, sources: &[&Source]) -> plan::InitPlan {
    plan::plan(request, sources).expect("the plan succeeds")
}

#[test]
fn plan_writes_the_selection_config() {
    let request = request(
        Language::Rust,
        &["gpui", "gpui-component"],
        &["core", "ui", "kit", "app", "cli"],
    );

    let plan = planned(&request, &[]);

    let expected = format!(
        "schema = 1\nversion = \"{VERSION}\"\nlanguage = \"rust\"\nlayers = [\"gpui\", \"gpui-component\"]\n\n[roles]\napp = \"crates/demo-app\"\ncli = \"crates/demo-cli\"\ncore = \"crates/demo-core\"\nkit = \"crates/demo-kit\"\nui = \"crates/demo-ui\"\n"
    );
    assert_eq!(plan.files["stomme.toml"], expected);
}

#[test]
fn plan_leaves_out_empty_layers_and_roles() {
    let request = request(Language::TypeScript, &[], &[]);

    let plan = planned(&request, &[]);

    let expected = format!("schema = 1\nversion = \"{VERSION}\"\nlanguage = \"typescript\"\n");
    assert_eq!(plan.files["stomme.toml"], expected);
}

#[test]
fn plan_fails_when_two_sources_own_one_file() {
    static FIRST: Source = Source {
        owner: "first",
        files: &[("README.md", "same\n")],
        merges: &[],
    };
    static SECOND: Source = Source {
        owner: "second",
        files: &[("README.md", "same\n")],
        merges: &[],
    };
    static CONFIG: Source = Source {
        owner: "first",
        files: &[("stomme.toml", "schema = 1\n")],
        merges: &[],
    };
    static MERGED: Source = Source {
        owner: "second",
        files: &[],
        merges: &[("README.md", "{}")],
    };
    let request = request(Language::Rust, &[], &[]);
    let cases: &[(&[&Source], &str, &str, &str)] = &[
        (&[&FIRST, &SECOND], "README.md", "first", "second"),
        (&[&CONFIG], "stomme.toml", "init", "first"),
        (&[&FIRST, &MERGED], "README.md", "first", "second"),
    ];

    for (sources, path, first, second) in cases {
        let error = plan::plan(&request, sources).expect_err("two owners clash");

        assert!(
            matches!(
                &error,
                InitError::OwnerClash { path: clash, first: one, second: two }
                    if clash == path && one == first && two == second
            ),
            "{error:?}"
        );
    }
}

#[test]
fn run_fails_before_writing_when_two_layers_set_one_key_differently() {
    static BASE: Source = Source {
        owner: "base",
        files: &[("README.md", "# Demo\n")],
        merges: &[("package.json", r#"{ "name": "demo" }"#)],
    };
    static FIRST_LAYER: Source = Source {
        owner: "first-layer",
        files: &[],
        merges: &[("package.json", r#"{ "scripts": { "test": "vp test" } }"#)],
    };
    static MIDDLE_LAYER: Source = Source {
        owner: "middle-layer",
        files: &[],
        merges: &[("package.json", r#"{ "scripts": { "check": "vp check" } }"#)],
    };
    static SECOND_LAYER: Source = Source {
        owner: "second-layer",
        files: &[],
        merges: &[("package.json", r#"{ "scripts": { "test": "vitest" } }"#)],
    };
    let root = empty_folder();
    let request = request(Language::Rust, &[], &[]);
    let sources = [&BASE, &FIRST_LAYER, &MIDDLE_LAYER, &SECOND_LAYER];

    let error = run_with_sources(root.path(), layer::catalog(), &request, &sources)
        .expect_err("the layers clash");

    assert!(
        matches!(
            &error,
            InitError::MergeClash { path, first, second }
                if path == "scripts.test" && first == "first-layer" && second == "second-layer"
        ),
        "{error:?}"
    );
    assert_eq!(entries(root.path()), Vec::<String>::new());
}

#[test]
fn plan_merges_equal_values_and_unions_arrays() {
    static FIRST: Source = Source {
        owner: "first",
        files: &[],
        merges: &[
            (
                "package.json",
                r#"{ "scripts": { "test": "vp test" }, "name": "demo", "keywords": ["a", "b"] }"#,
            ),
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/demo\"]\nresolver = \"3\"\n",
            ),
        ],
    };
    static SECOND: Source = Source {
        owner: "second",
        files: &[],
        merges: &[
            (
                "package.json",
                r#"{ "name": "demo", "keywords": ["c", "b"], "scripts": { "check": "vp check" } }"#,
            ),
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"xtask\", \"crates/demo\"]\nresolver = \"3\"\n",
            ),
        ],
    };
    let request = request(Language::Rust, &[], &[]);

    let plan = planned(&request, &[&FIRST, &SECOND]);

    let package = "{\n  \"keywords\": [\n    \"a\",\n    \"b\",\n    \"c\"\n  ],\n  \"name\": \"demo\",\n  \"scripts\": {\n    \"check\": \"vp check\",\n    \"test\": \"vp test\"\n  }\n}\n";
    let cargo = "[workspace]\nmembers = [\"crates/demo\", \"xtask\"]\nresolver = \"3\"\n";
    assert_eq!(plan.files["package.json"], package);
    assert_eq!(plan.files["Cargo.toml"], cargo);
}

#[test]
fn plan_reports_a_toml_clash_with_its_dotted_path() {
    static FIRST: Source = Source {
        owner: "first",
        files: &[],
        merges: &[("Cargo.toml", "[workspace]\nresolver = \"2\"\n")],
    };
    static SECOND: Source = Source {
        owner: "second",
        files: &[],
        merges: &[("Cargo.toml", "[workspace]\nresolver = \"3\"\n")],
    };
    let request = request(Language::Rust, &[], &[]);

    let error = plan::plan(&request, &[&FIRST, &SECOND]).expect_err("the fragments clash");

    assert!(
        matches!(
            &error,
            InitError::MergeClash { path, first, second }
                if path == "workspace.resolver" && first == "first" && second == "second"
        ),
        "{error:?}"
    );
}

#[test]
fn merge_reports_a_root_clash_with_the_target() {
    let fragments = [("first", "[1]"), ("second", "{}")];

    let error = merge::merge("data.json", &fragments).expect_err("the roots clash");

    assert!(
        matches!(
            &error,
            InitError::MergeClash { path, first, second }
                if path == "data.json" && first == "first" && second == "second"
        ),
        "{error:?}"
    );
}

#[test]
fn merge_rejects_a_target_that_is_not_json_or_toml() {
    let error = merge::merge("README.md", &[("first", "# Demo\n")]).expect_err("no merge format");

    assert!(
        matches!(&error, InitError::MergeTarget { path } if path == "README.md"),
        "{error:?}"
    );
}

#[test]
fn merge_rejects_a_fragment_that_does_not_parse() {
    let cases = [
        ("package.json", "{}", "{"),
        ("Cargo.toml", "", "[workspace"),
    ];

    for (target, valid, invalid) in cases {
        let fragments = [("first", valid), ("second", invalid)];

        let error = merge::merge(target, &fragments).expect_err("the fragment is invalid");

        assert!(
            matches!(
                &error,
                InitError::InvalidFragment { path, owner, .. } if path == target && owner == "second"
            ),
            "{error:?}"
        );
    }
}

#[test]
fn plan_substitutes_name_and_version() {
    static SOURCE: Source = Source {
        owner: "first",
        files: &[("crates/{{name}}/VERSION", "{{name}} {{version}}\n")],
        merges: &[(
            "crates/{{name}}/Cargo.toml",
            "[package]\nname = \"{{name}}\"\n",
        )],
    };
    let request = request(Language::Rust, &[], &[]);

    let plan = planned(&request, &[&SOURCE]);

    assert_eq!(
        plan.files["crates/demo/VERSION"],
        format!("demo {VERSION}\n")
    );
    assert_eq!(
        plan.files["crates/demo/Cargo.toml"],
        "[package]\nname = \"demo\"\n"
    );
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
fn run_writes_nested_files_and_merge_targets() {
    static BASE: Source = Source {
        owner: "base",
        files: &[(
            "crates/{{name}}/src/lib.rs",
            "pub fn answer() -> u8 {\n    42\n}\n",
        )],
        merges: &[(
            "crates/{{name}}/config/settings.toml",
            "[package]\nname = \"{{name}}\"\n",
        )],
    };
    static LAYER: Source = Source {
        owner: "layer",
        files: &[],
        merges: &[(
            "crates/{{name}}/config/settings.toml",
            "[package]\nedition = \"2024\"\n",
        )],
    };
    let root = empty_folder();
    let request = request(Language::Rust, &[], &[]);

    run_with_sources(root.path(), layer::catalog(), &request, &[&BASE, &LAYER])
        .expect("init succeeds");

    let crate_folder = root.path().join("crates/demo");
    assert!(crate_folder.join("src").is_dir());
    assert!(crate_folder.join("config").is_dir());
    assert_eq!(
        fs::read_to_string(crate_folder.join("src/lib.rs")).expect("read lib.rs"),
        "pub fn answer() -> u8 {\n    42\n}\n"
    );
    assert_eq!(
        fs::read_to_string(crate_folder.join("config/settings.toml")).expect("read settings"),
        "[package]\nedition = \"2024\"\nname = \"demo\"\n"
    );
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
fn every_catalog_layer_has_a_starter_table() {
    for (name, _) in layer::catalog().guides() {
        assert!(assets::layer(name).is_some(), "{name}");
    }
}

fn common_plan() -> plan::InitPlan {
    let request = request(Language::Rust, &[], &[]);

    planned(&request, &[assets::sources(&request).unwrap()[0]])
}

#[test]
fn plan_writes_the_common_files() {
    let plan = common_plan();

    for path in [
        "LICENSE",
        "AGENTS.md",
        "docs/README.md",
        "docs/adr/README.md",
        "docs/adr/TEMPLATE.md",
        "renovate.json",
        ".github/workflows/sync-update.yml",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }
}

#[test]
fn plan_pins_the_sync_workflow_to_the_crate_version() {
    let plan = common_plan();
    let workflow = &plan.files[".github/workflows/sync-update.yml"];

    assert!(workflow.contains(&format!("sync-update.yml@v{VERSION}")));
    assert!(!workflow.contains("{{version}}"));
}

#[test]
fn plan_substitutes_the_project_name() {
    let plan = common_plan();

    for path in ["LICENSE", "AGENTS.md"] {
        assert!(plan.files[path].contains("demo"), "{path} lacks the name");
        assert!(
            !plan.files[path].contains("{{name}}"),
            "{path} keeps a placeholder"
        );
    }

    for (path, text) in &plan.files {
        let without_expressions = text.replace("${{", "");

        assert!(
            !without_expressions.contains("{{"),
            "{path} keeps a placeholder"
        );
    }
}

#[test]
fn plan_writes_every_common_required_part() {
    let plan = common_plan();

    for path in [
        "docs/README.md",
        "docs/adr/README.md",
        "docs/adr/TEMPLATE.md",
        "AGENTS.md",
        "renovate.json",
        "LICENSE",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }

    for path in ["docs/development.md", ".pi/tau.json"] {
        assert!(!plan.files.contains_key(path), "common owns {path}");
    }
}
