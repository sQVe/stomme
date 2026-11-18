use std::fs;

use super::super::assets::{self, Source};
use super::super::{InitError, empty_folder, planned, request};
use super::super::{plan, run_with_sources};
use crate::layer::{self, Language};

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
fn plan_substitutes_the_project_name() {
    let request = request(Language::Rust, &[], &[]);
    let sources = assets::sources(&request, layer::catalog()).expect("the sources resolve");
    let refs: Vec<&Source> = sources.iter().collect();

    let plan = planned(&request, &refs);

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

    run_with_sources(
        root.path(),
        layer::catalog(),
        &request,
        &[&BASE, &LAYER],
        &crate::config::FileRead::filesystem(),
    )
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
