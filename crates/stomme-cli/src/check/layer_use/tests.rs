use std::path::Path;

use super::*;
use crate::check::cargo_lock;
use crate::check::test_support::{lock_text, project, project_with_language};
use crate::config::{self, Installation};
use crate::layer;
use crate::rule::Rule;

fn problems(root: &Path) -> Vec<Problem> {
    let Installation::Valid(selection) =
        config::read(root, layer::catalog(), &config::FileRead::filesystem())
    else {
        panic!("expected a valid installation");
    };
    let lock = match cargo_lock::read(root).expect("read the lock") {
        Ok(lock) => lock,
        Err(problem) => return vec![problem],
    };
    let package = crate::check::package_json::read(root).expect("read package.json");
    let package = package.as_ref().and_then(|result| result.as_ref().ok());

    check(root, &selection, layer::catalog(), lock.as_ref(), package).expect("check the project")
}

fn layer_problems(root: &Path) -> Vec<Problem> {
    problems(root)
        .into_iter()
        .filter(|problem| problem.rule == Rule::LayerDeclared)
        .collect()
}

#[test]
fn rust_project_with_gpui_in_the_lock_and_no_gpui_layer_is_reported() {
    let lock = lock_text(&[("gpui", "0.0.0", &[])]);
    let root = project(&[("Cargo.lock", &lock)], "");

    let problems = layer_problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, "Cargo.lock");
    assert_eq!(problems[0].key.as_deref(), Some("gpui"));
    assert!(problems[0].message.contains("gpui"), "{problems:?}");
    assert!(problems[0].message.contains("layers"), "{problems:?}");
}

#[test]
fn rust_project_with_gpui_component_and_no_gpui_component_layer_is_reported() {
    let lock = lock_text(&[("gpui", "0.0.0", &[]), ("gpui-component", "0.0.0", &[])]);
    let root = project(&[("Cargo.lock", &lock)], "layers = [\"gpui\"]\n");

    let problems = layer_problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].key.as_deref(), Some("gpui-component"));
}

#[test]
fn typescript_project_with_react_and_no_react_layer_is_reported() {
    for field in ["dependencies", "devDependencies", "peerDependencies"] {
        let package = format!("{{\"{field}\": {{\"react\": \"19.0.0\"}}}}");
        let root = project_with_language("typescript", &[("package.json", &package)], "");

        let problems = layer_problems(root.path());

        assert_eq!(problems.len(), 1, "{field}: {problems:?}");
        assert_eq!(problems[0].path, "package.json", "{field}");
        assert_eq!(problems[0].key.as_deref(), Some("react"), "{field}");
    }
}

#[test]
fn each_framework_passes_once_its_layer_is_listed() {
    let gpui_lock = lock_text(&[("gpui", "0.0.0", &[])]);
    let gpui = project(&[("Cargo.lock", &gpui_lock)], "layers = [\"gpui\"]\n");

    assert_eq!(layer_problems(gpui.path()), []);

    let component_lock = lock_text(&[("gpui", "0.0.0", &[]), ("gpui-component", "0.0.0", &[])]);
    let component = project(
        &[("Cargo.lock", &component_lock)],
        "layers = [\"gpui\", \"gpui-component\"]\n",
    );

    assert_eq!(layer_problems(component.path()), []);

    for field in ["dependencies", "devDependencies", "peerDependencies"] {
        let package = format!("{{\"{field}\": {{\"react\": \"19.0.0\"}}}}");
        let root = project_with_language(
            "typescript",
            &[("package.json", &package)],
            "layers = [\"react\"]\n",
        );

        assert_eq!(layer_problems(root.path()), [], "{field}");
    }
}

#[test]
fn pi_field_without_the_pi_layer_is_reported() {
    let package = r#"{"pi": {"extensions": []}}"#;
    let root = project_with_language("typescript", &[("package.json", package)], "");

    let problems = layer_problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, "package.json");
    assert_eq!(problems[0].key.as_deref(), Some("pi"));

    let with_layer = project_with_language(
        "typescript",
        &[("package.json", package)],
        "layers = [\"pi\"]\n",
    );

    assert_eq!(layer_problems(with_layer.path()), []);
}

#[test]
fn herdr_mod_without_the_herdr_layer_is_reported() {
    let files = [
        ("src/mods/menu/main.ts", "export {};\n"),
        ("src/mods/menu/mod.json", "{}\n"),
    ];
    let root = project_with_language("typescript", &files, "");

    let problems = layer_problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, "src/mods/menu/mod.json");
    assert_eq!(problems[0].key.as_deref(), Some("herdr"));

    let with_layer = project_with_language("typescript", &files, "layers = [\"herdr\"]\n");

    assert_eq!(layer_problems(with_layer.path()), []);
}

#[test]
fn herdr_gpui_kit_use_passes_without_the_gpui_layer() {
    let files = [
        (
            "src/mods/menu/main.ts",
            "import { render } from \"gpui-kit\";\n",
        ),
        ("src/mods/menu/mod.json", "{}\n"),
        ("herdr/gpui-kit.d.ts", "export {};\n"),
    ];
    let root = project_with_language("typescript", &files, "layers = [\"herdr\"]\n");

    assert_eq!(layer_problems(root.path()), []);
}

#[test]
fn mod_json_without_a_main_file_beside_it_is_not_herdr() {
    let root = project_with_language("typescript", &[("src/mods/menu/mod.json", "{}\n")], "");

    assert_eq!(layer_problems(root.path()), []);
}

#[test]
fn two_herdr_mods_report_one_problem_at_the_first_mod() {
    let files = [
        ("src/mods/menu/main.ts", "export {};\n"),
        ("src/mods/menu/mod.json", "{}\n"),
        ("src/mods/panel/main.ts", "export {};\n"),
        ("src/mods/panel/mod.json", "{}\n"),
    ];
    let root = project_with_language("typescript", &files, "");

    let problems = layer_problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, "src/mods/menu/mod.json");
}

#[test]
fn missing_manifests_report_nothing() {
    let rust = project(&[], "");

    assert_eq!(layer_problems(rust.path()), []);

    let typescript = project_with_language("typescript", &[], "");

    assert_eq!(layer_problems(typescript.path()), []);
}

#[test]
fn git_sourced_gpui_without_the_gpui_layer_is_reported() {
    let lock = "version = 4\n\n[[package]]\nname = \"gpui\"\nversion = \"0.0.0\"\nsource = \"git+https://github.com/zed-industries/zed\"\n";
    let root = project(&[("Cargo.lock", lock)], "");

    let problems = layer_problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].key.as_deref(), Some("gpui"));
}
