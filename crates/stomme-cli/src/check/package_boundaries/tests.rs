use std::path::Path;

use super::*;
use crate::check::test_support::{lock_text, project};
use crate::config::{self, Installation};
use crate::layer;

fn problems(root: &Path) -> Vec<Problem> {
    let Installation::Valid(selection) = config::read(root, layer::catalog()) else {
        panic!("expected a valid installation");
    };

    check(root, &selection).expect("check the project")
}

fn manifest(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\n")
}

#[test]
fn core_package_with_a_transitive_gpui_dependency_names_the_chain_and_layer() {
    let core = manifest("greppa-core");
    let lock = lock_text(&[
        ("greppa-core", "0.0.0", &["serde"]),
        ("serde", "1.0.0", &["gpui_macros"]),
        ("gpui_macros", "0.1.0", &[]),
    ]);
    let root = project(
        &[
            ("crates/greppa-core/Cargo.toml", &core),
            ("Cargo.lock", &lock),
        ],
        "layers = [\"gpui\"]\n\n[roles]\ncore = \"crates/greppa-core\"\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].rule, Rule::GpuiPlainCore);
    assert_eq!(problems[0].path, "crates/greppa-core");
    assert_eq!(problems[0].key.as_deref(), Some("core:gpui_macros"));
    assert!(problems[0].message.contains("greppa-core"), "{problems:?}");
    assert!(problems[0].message.contains("gpui_macros"), "{problems:?}");
    assert!(
        problems[0]
            .message
            .contains("greppa-core -> serde -> gpui_macros"),
        "{problems:?}"
    );
    assert!(problems[0].message.contains("layer `gpui`"), "{problems:?}");
}

#[test]
fn cli_package_that_depends_on_the_ui_role_package_is_reported() {
    let cli = manifest("greppa-cli");
    let ui = manifest("greppa-ui");
    let lock = lock_text(&[
        ("greppa-cli", "0.0.0", &["greppa-ui"]),
        ("greppa-ui", "0.0.0", &[]),
    ]);
    let root = project(
        &[
            ("crates/greppa-cli/Cargo.toml", &cli),
            ("crates/greppa-ui/Cargo.toml", &ui),
            ("Cargo.lock", &lock),
        ],
        "layers = [\"gpui\"]\n\n[roles]\ncli = \"crates/greppa-cli\"\nui = \"crates/greppa-ui\"\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].rule, Rule::GpuiPlainCore);
    assert_eq!(problems[0].key.as_deref(), Some("cli:greppa-ui"));
}

#[test]
fn clean_split_reports_nothing() {
    let core = manifest("greppa-core");
    let lock = lock_text(&[
        ("greppa-core", "0.0.0", &["serde"]),
        ("serde", "1.0.0", &[]),
    ]);
    let root = project(
        &[
            ("crates/greppa-core/Cargo.toml", &core),
            ("Cargo.lock", &lock),
        ],
        "layers = [\"gpui\"]\n\n[roles]\ncore = \"crates/greppa-core\"\n",
    );

    assert_eq!(problems(root.path()), []);
}

#[test]
fn project_without_the_gpui_layer_reads_no_lock() {
    let core = manifest("greppa-core");
    let root = project(
        &[("crates/greppa-core/Cargo.toml", &core)],
        "[roles]\ncore = \"crates/greppa-core\"\n",
    );

    assert_eq!(problems(root.path()), []);
}

#[test]
fn role_package_outside_kit_that_depends_on_gpui_component_is_reported() {
    let ui = manifest("greppa-ui");
    let kit = manifest("greppa-kit");
    let lock = lock_text(&[
        ("greppa-ui", "0.0.0", &["gpui-component"]),
        ("greppa-kit", "0.0.0", &["gpui-component"]),
        ("gpui-component", "0.0.0", &[]),
    ]);
    let root = project(
        &[
            ("crates/greppa-ui/Cargo.toml", &ui),
            ("crates/greppa-kit/Cargo.toml", &kit),
            ("Cargo.lock", &lock),
        ],
        "layers = [\"gpui\", \"gpui-component\"]\n\n[roles]\nui = \"crates/greppa-ui\"\nkit = \"crates/greppa-kit\"\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].rule, Rule::GpuiComponentKit);
    assert_eq!(problems[0].path, "Cargo.lock");
    assert_eq!(problems[0].key.as_deref(), Some("greppa-ui"));
    assert!(problems[0].message.contains("greppa-kit"), "{problems:?}");
    assert!(
        problems[0].message.contains("layer `gpui-component`"),
        "{problems:?}"
    );
}

#[test]
fn kit_package_may_depend_on_gpui_component() {
    let ui = manifest("greppa-ui");
    let kit = manifest("greppa-kit");
    let lock = lock_text(&[
        ("greppa-ui", "0.0.0", &["serde"]),
        ("greppa-kit", "0.0.0", &["gpui-component"]),
        ("gpui-component", "0.0.0", &[]),
        ("serde", "1.0.0", &[]),
    ]);
    let root = project(
        &[
            ("crates/greppa-ui/Cargo.toml", &ui),
            ("crates/greppa-kit/Cargo.toml", &kit),
            ("Cargo.lock", &lock),
        ],
        "layers = [\"gpui\", \"gpui-component\"]\n\n[roles]\nui = \"crates/greppa-ui\"\nkit = \"crates/greppa-kit\"\n",
    );

    assert_eq!(problems(root.path()), []);
}

#[test]
fn project_without_a_kit_role_is_told_to_add_one() {
    let ui = manifest("greppa-ui");
    let lock = lock_text(&[
        ("greppa-ui", "0.0.0", &["gpui-component"]),
        ("gpui-component", "0.0.0", &[]),
    ]);
    let root = project(
        &[("crates/greppa-ui/Cargo.toml", &ui), ("Cargo.lock", &lock)],
        "layers = [\"gpui\", \"gpui-component\"]\n\n[roles]\nui = \"crates/greppa-ui\"\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].rule, Rule::GpuiComponentKit);
    assert!(
        problems[0].message.contains("add a `kit` role"),
        "{problems:?}"
    );
}

#[test]
fn package_outside_the_roles_is_not_checked() {
    let ui = manifest("greppa-ui");
    let lock = lock_text(&[
        ("greppa-ui", "0.0.0", &["serde"]),
        ("serde", "1.0.0", &[]),
        ("vendored", "0.0.0", &["gpui-component"]),
        ("gpui-component", "0.0.0", &[]),
    ]);
    let root = project(
        &[("crates/greppa-ui/Cargo.toml", &ui), ("Cargo.lock", &lock)],
        "layers = [\"gpui\", \"gpui-component\"]\n\n[roles]\nui = \"crates/greppa-ui\"\n",
    );

    assert_eq!(problems(root.path()), []);
}

#[test]
fn missing_lock_with_the_gpui_layer_is_a_config_problem() {
    let core = manifest("greppa-core");
    let root = project(
        &[("crates/greppa-core/Cargo.toml", &core)],
        "layers = [\"gpui\"]\n\n[roles]\ncore = \"crates/greppa-core\"\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].rule, Rule::RustConfigFile);
    assert_eq!(problems[0].path, "Cargo.lock");
    assert!(problems[0].message.contains("is missing"), "{problems:?}");
}

#[test]
fn malformed_lock_is_a_config_problem() {
    let core = manifest("greppa-core");
    let cases = [
        (
            "missing name",
            "version = 4\n\n[[package]]\nversion = \"0.0.0\"\n",
            "missing field `name`",
        ),
        (
            "missing version",
            "version = 4\n\n[[package]]\nname = \"greppa-core\"\n",
            "missing field `version`",
        ),
        (
            "ambiguous dependency",
            "version = 4\n\n[[package]]\nname = \"greppa-core\"\nversion = \"0.0.0\"\ndependencies = [\"foo\"]\n\n[[package]]\nname = \"foo\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"foo\"\nversion = \"2.0.0\"\n",
            "matches more than one lock package",
        ),
    ];

    for (case, lock, expected) in cases {
        let root = project(
            &[
                ("crates/greppa-core/Cargo.toml", &core),
                ("Cargo.lock", lock),
            ],
            "layers = [\"gpui\"]\n\n[roles]\ncore = \"crates/greppa-core\"\n",
        );

        let problems = problems(root.path());

        assert_eq!(problems.len(), 1, "{case}: {problems:?}");
        assert_eq!(problems[0].rule, Rule::RustConfigFile, "{case}");
        assert_eq!(problems[0].path, "Cargo.lock", "{case}");
        assert!(
            problems[0].message.contains(expected),
            "{case}: {problems:?}"
        );
    }
}

#[test]
fn role_package_missing_from_the_lock_is_a_config_problem() {
    let core = manifest("greppa-core");
    let lock = lock_text(&[("serde", "1.0.0", &[])]);
    let root = project(
        &[
            ("crates/greppa-core/Cargo.toml", &core),
            ("Cargo.lock", &lock),
        ],
        "layers = [\"gpui\"]\n\n[roles]\ncore = \"crates/greppa-core\"\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].rule, Rule::RustConfigFile);
    assert_eq!(problems[0].path, "Cargo.lock");
    assert!(problems[0].message.contains("greppa-core"), "{problems:?}");
}
