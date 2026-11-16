use serde_json::json;

use super::*;
use crate::rule::Rule;
use crate::test_support::write_file;

fn project_with(manifest: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), PACKAGE_JSON, manifest);

    root
}

fn found(problems: &[Problem]) -> Vec<(&str, Rule)> {
    problems
        .iter()
        .map(|problem| (problem.path.as_str(), problem.rule))
        .collect()
}

fn pinned_version(package: &str) -> &str {
    pins()
        .get(package)
        .unwrap_or_else(|| panic!("packages.toml pins {package}"))
}

#[test]
fn check_reports_a_pinned_package_at_another_version() {
    let root = project_with(
        &json!({
            "dependencies": { "@sqve/mutation": "0.1.1" },
        })
        .to_string(),
    );

    let problems = check(root.path()).expect("check the project");

    assert_eq!(found(&problems), [(PACKAGE_JSON, Rule::PackagePin)]);
    let message = &problems[0].message;
    assert!(message.contains("@sqve/mutation"), "{message}");
    assert!(message.contains("0.1.1"), "{message}");
    assert!(
        message.contains(pinned_version("@sqve/mutation")),
        "{message}"
    );
}

#[test]
fn check_reports_a_version_range_for_a_pinned_package() {
    let pinned = pinned_version("@sqve/stomme-ts-style");
    let root = project_with(
        &json!({
            "devDependencies": { "@sqve/stomme-ts-style": format!("^{pinned}") },
        })
        .to_string(),
    );

    let problems = check(root.path()).expect("check the project");

    assert_eq!(found(&problems), [(PACKAGE_JSON, Rule::PackagePin)]);
    assert!(
        problems[0].message.contains("@sqve/stomme-ts-style"),
        "{problems:?}"
    );
    assert!(problems[0].message.contains(pinned), "{problems:?}");
}

#[test]
fn check_passes_the_pinned_versions_in_dependencies_and_dev_dependencies() {
    let names: Vec<&String> = pins().keys().collect();
    let split = names.len().div_ceil(2);
    let dependencies: BTreeMap<&String, &String> = names[..split]
        .iter()
        .map(|name| (*name, &pins()[*name]))
        .collect();
    let dev_dependencies: BTreeMap<&String, &String> = names[split..]
        .iter()
        .map(|name| (*name, &pins()[*name]))
        .collect();
    let root = project_with(
        &json!({
            "dependencies": dependencies,
            "devDependencies": dev_dependencies,
        })
        .to_string(),
    );

    assert_eq!(check(root.path()).expect("check the project"), []);
}

#[test]
fn check_passes_when_a_pinned_package_is_absent() {
    let root = project_with(&json!({ "dependencies": { "left-pad": "1.3.0" } }).to_string());

    assert_eq!(check(root.path()).expect("check the project"), []);
}

#[test]
fn check_reports_a_manifest_that_is_not_valid_json() {
    let root = project_with("{ not json");

    let problems = check(root.path()).expect("check the project");

    assert_eq!(found(&problems), [(PACKAGE_JSON, Rule::PackagePin)]);
    assert!(
        problems[0].message.contains("is not valid JSON"),
        "{problems:?}"
    );
}

#[test]
fn check_reports_dependencies_that_are_not_an_object() {
    let root = project_with(&json!({ "dependencies": "not an object" }).to_string());

    let problems = check(root.path()).expect("check the project");

    assert_eq!(found(&problems), [(PACKAGE_JSON, Rule::PackagePin)]);
    assert!(
        problems[0].message.contains("is not valid JSON"),
        "{problems:?}"
    );
}

#[test]
fn check_passes_without_a_package_json() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    assert_eq!(check(root.path()).expect("check the project"), []);
}
