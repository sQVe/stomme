use super::super::assets::Source;
use super::super::plan;
use super::super::{
    InitError, empty_folder, entries, planned, request, run_with_sources, test_commit,
};
use super::merge;
use crate::layer::{self, Language};

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

    let error = run_with_sources(
        root.path(),
        layer::catalog(),
        &request,
        &sources,
        &crate::config::FileRead::filesystem(),
        || Ok(test_commit()),
    )
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

    let error =
        plan::plan(&request, &[&FIRST, &SECOND], &test_commit()).expect_err("the fragments clash");

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

    let error = merge("data.json", &fragments).expect_err("the roots clash");

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
    let error = merge("README.md", &[("first", "# Demo\n")]).expect_err("no merge format");

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

        let error = merge(target, &fragments).expect_err("the fragment is invalid");

        assert!(
            matches!(
                &error,
                InitError::InvalidFragment { path, owner, .. } if path == target && owner == "second"
            ),
            "{error:?}"
        );
    }
}
