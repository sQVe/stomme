use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;
use crate::packages::pins;
use crate::test_support::write_file;

fn project_with(manifest: &str) -> TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), PACKAGE_JSON, manifest);

    root
}

fn planned_text(root: &Path) -> String {
    let (path, text) = plan(root)
        .expect("plan the package pins")
        .expect("a write is planned");

    assert_eq!(path, PACKAGE_JSON);

    text
}

fn pinned(package: &str) -> String {
    pins()
        .get(package)
        .unwrap_or_else(|| panic!("packages.toml pins {package}"))
        .clone()
}

#[test]
fn sync_sets_a_declared_stomme_package_to_its_pinned_version() {
    let root = project_with(
        &json!({
            "name": "fixture",
            "version": "1.0.0",
            "scripts": { "test": "true" },
            "devDependencies": {
                "@sqve/stomme-ts-style": "0.0.1",
                "left-pad": "1.0.0",
            },
        })
        .to_string(),
    );

    let text = planned_text(root.path());
    let manifest: Value = serde_json::from_str(&text).expect("the planned text is JSON");
    let object = manifest.as_object().expect("the manifest is an object");
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();

    assert_eq!(keys, ["name", "version", "scripts", "devDependencies"]);
    let dev_dependencies = object["devDependencies"]
        .as_object()
        .expect("devDependencies is an object");
    let dev_keys: Vec<&str> = dev_dependencies.keys().map(String::as_str).collect();

    assert_eq!(dev_keys, ["@sqve/stomme-ts-style", "left-pad"]);
    assert_eq!(
        dev_dependencies["@sqve/stomme-ts-style"],
        Value::String(pinned("@sqve/stomme-ts-style"))
    );
    assert_eq!(
        dev_dependencies["left-pad"],
        Value::String("1.0.0".to_string())
    );
    assert!(text.ends_with("}\n"), "{text}");
}

#[test]
fn sync_plans_nothing_when_the_pins_already_match() {
    let root = project_with(&json!({ "dependencies": pins() }).to_string());

    assert_eq!(plan(root.path()).expect("plan the package pins"), None);
}

#[test]
fn sync_plans_nothing_without_a_package_json() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    assert_eq!(plan(root.path()).expect("plan the package pins"), None);
}

#[test]
fn sync_never_adds_an_undeclared_stomme_package() {
    let root = project_with(&json!({ "dependencies": { "left-pad": "1.0.0" } }).to_string());

    assert_eq!(plan(root.path()).expect("plan the package pins"), None);
}

#[test]
fn sync_refuses_a_package_json_that_is_not_valid_json() {
    let root = project_with("{ not json");

    let error = plan(root.path()).expect_err("plan refuses");

    assert!(
        matches!(error, SyncError::PackageManifest { ref reason } if reason.contains("is not valid JSON")),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_a_stomme_package_whose_version_is_not_a_string() {
    let root =
        project_with(&json!({ "devDependencies": { "@sqve/stomme-ts-style": 1 } }).to_string());

    let error = plan(root.path()).expect_err("plan refuses");

    assert!(
        matches!(error, SyncError::PackageManifest { ref reason } if reason.contains("@sqve/stomme-ts-style")),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_a_package_json_whose_root_is_not_an_object() {
    let root = project_with("[]");

    let error = plan(root.path()).expect_err("plan refuses");

    assert!(
        matches!(error, SyncError::PackageManifest { ref reason } if reason.contains("not a JSON object")),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_dependencies_that_are_not_an_object() {
    let root = project_with(&json!({ "dependencies": "not an object" }).to_string());

    let error = plan(root.path()).expect_err("plan refuses");

    assert!(
        matches!(error, SyncError::PackageManifest { ref reason } if reason.contains("dependencies")),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_dev_dependencies_that_are_not_an_object() {
    let root = project_with(&json!({ "devDependencies": "not an object" }).to_string());

    let error = plan(root.path()).expect_err("plan refuses");

    assert!(
        matches!(error, SyncError::PackageManifest { ref reason } if reason.contains("devDependencies")),
        "{error:?}"
    );
}

#[test]
fn sync_writes_pins_and_a_second_sync_changes_nothing() {
    let root = crate::test_support::complete_project();
    let manifest = json!({
        "name": "fixture",
        "devDependencies": { "@sqve/stomme-ts-style": "0.0.1" },
    });

    write_file(root.path(), PACKAGE_JSON, &manifest.to_string());
    crate::sync_project(root.path());

    let target = crate::check::TargetBranch::git("origin/main".to_string());
    let problems = crate::check::run(
        root.path(),
        crate::layer::catalog(),
        &target,
        &crate::config::FileRead::filesystem(),
    )
    .expect("check the project");

    assert!(
        !problems
            .iter()
            .any(|problem| problem.rule == crate::rule::Rule::PackagePin),
        "{problems:?}"
    );

    let first = fs::read(root.path().join(PACKAGE_JSON)).expect("read package.json");

    crate::sync_project(root.path());

    let second = fs::read(root.path().join(PACKAGE_JSON)).expect("read package.json");

    assert_eq!(second, first);
    let written: Value = serde_json::from_slice(&first).expect("package.json is JSON");

    assert_eq!(
        written["devDependencies"]["@sqve/stomme-ts-style"],
        Value::String(pinned("@sqve/stomme-ts-style"))
    );
}

#[test]
fn sync_with_an_invalid_package_json_writes_only_the_lock_file() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let config = format!(
        "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n",
        env!("CARGO_PKG_VERSION")
    );

    write_file(root.path(), "stomme.toml", &config);
    write_file(root.path(), PACKAGE_JSON, "{ not json");

    let error = crate::sync::run(
        root.path(),
        crate::layer::catalog(),
        &crate::config::FileRead::filesystem(),
    )
    .expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::PackageManifest { .. }),
        "{error:?}"
    );

    let entries = fs::read_dir(root.path().join("docs/stomme")).expect("read the synced folder");
    let names: Vec<String> = entries
        .map(|entry| {
            entry
                .expect("read an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    assert_eq!(
        names,
        [crate::sync::LOCK_NAME],
        "the run writes no other file"
    );
    assert!(!root.path().join(crate::sync::RECORD_FILE).exists());
}
