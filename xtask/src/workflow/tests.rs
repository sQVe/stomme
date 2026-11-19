use super::*;

use serde_json::Value;

const TOKEN_ACTION: &str = "actions/create-github-app-token@64ab";
const RELEASE_GATE: &str = include_str!("../../../.github/workflows/release-gate.yml");
const RENOVATE_PRESET: &str = include_str!("../../../renovate/stomme.json");
const STOMME_PACKAGE: &str = "sQVe/stomme";
const BARE_VERSION_EXTRACTION: &str = "^v(?<version>.*)$";
const STOMME_TOML_FILE_PATTERN: &str = r"/^stomme\.toml$/";
const WORKFLOW_FILE_PATTERN: &str = r"/^\.github\/workflows\/[^/]+\.ya?ml$/";

fn token_step(action: &str, permissions: &[&str]) -> String {
    let mut step = format!("      - uses: {action}\n        with:\n          app-id: app\n");

    for permission in permissions {
        step.push_str("          ");
        step.push_str(permission);
        step.push('\n');
    }

    step
}

#[test]
fn permission_problems_decides_from_permission_lines() {
    let cases: [(&str, String, bool); 10] = [
        (
            "contents read",
            token_step(TOKEN_ACTION, &["permission-contents: read"]),
            true,
        ),
        ("no permission input", token_step(TOKEN_ACTION, &[]), false),
        (
            "pull requests write",
            token_step(
                TOKEN_ACTION,
                &[
                    "permission-contents: read",
                    "permission-pull-requests: write",
                ],
            ),
            false,
        ),
        (
            "contents write",
            token_step(TOKEN_ACTION, &["permission-contents: write"]),
            false,
        ),
        (
            "quoted read",
            token_step(TOKEN_ACTION, &["permission-contents: 'read'"]),
            true,
        ),
        (
            "trailing comment",
            token_step(TOKEN_ACTION, &["permission-contents: read # why"]),
            true,
        ),
        (
            "quoted key",
            token_step(TOKEN_ACTION, &["\"permission-contents\": read"]),
            true,
        ),
        (
            "quoted action without permission",
            token_step("\"actions/create-github-app-token@64ab\"", &[]),
            false,
        ),
        (
            "two token steps with one permission",
            format!(
                "{}{}",
                token_step(TOKEN_ACTION, &["permission-contents: read"]),
                token_step(TOKEN_ACTION, &[])
            ),
            false,
        ),
        (
            "expression value",
            token_step(TOKEN_ACTION, &["permission-contents: ${{ x }}"]),
            false,
        ),
    ];

    for (name, workflow, accepted) in cases {
        let problems = permission_problems(&workflow);

        assert_eq!(problems.is_empty(), accepted, "{name}: {problems:?}");
    }
}

#[test]
fn release_gate_token_steps_request_read_only_contents() {
    let token_steps = RELEASE_GATE
        .lines()
        .filter(|line| line.contains("actions/create-github-app-token"))
        .count();

    assert!(token_steps > 0, "the release gate creates an app token");
    assert_eq!(permission_problems(RELEASE_GATE), Vec::<String>::new());
}

#[test]
fn renovate_preset_mise_rule_extracts_bare_versions_for_the_github_backend() {
    let preset = renovate_preset();
    let rule = package_rule(&preset, "mise");

    assert_eq!(string_list(rule, "matchDepNames"), ["github:sQVe/stomme"]);
    assert_eq!(
        rule["extractVersion"].as_str(),
        Some(BARE_VERSION_EXTRACTION)
    );
}

#[test]
fn renovate_preset_stomme_toml_manager_extracts_bare_versions() {
    let preset = renovate_preset();
    let manager = custom_manager(&preset, STOMME_TOML_FILE_PATTERN);

    assert_eq!(
        manager["extractVersionTemplate"].as_str(),
        Some(BARE_VERSION_EXTRACTION)
    );
}

#[test]
fn renovate_preset_sync_workflow_manager_extracts_bare_versions() {
    let preset = renovate_preset();
    let manager = custom_manager(&preset, WORKFLOW_FILE_PATTERN);

    assert_eq!(
        manager["extractVersionTemplate"].as_str(),
        Some(BARE_VERSION_EXTRACTION)
    );
}

#[test]
fn renovate_preset_sync_workflow_manager_writes_the_v_tag_prefix() {
    let preset = renovate_preset();
    let manager = custom_manager(&preset, WORKFLOW_FILE_PATTERN);
    let template = manager["autoReplaceStringTemplate"].as_str();

    assert!(
        template.is_some_and(|template| template.ends_with("@v{{newValue}}")),
        "template: {template:?}"
    );
}

#[test]
fn renovate_preset_disables_the_github_actions_manager_for_stomme() {
    let preset = renovate_preset();
    let rule = package_rule(&preset, "github-actions");

    assert_eq!(rule["enabled"].as_bool(), Some(false));
}

fn renovate_preset() -> Value {
    serde_json::from_str(RENOVATE_PRESET).expect("the Renovate preset is valid JSON")
}

fn string_list<'a>(value: &'a Value, key: &str) -> Vec<&'a str> {
    value[key]
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn package_rule<'a>(preset: &'a Value, manager: &str) -> &'a Value {
    let rules = preset["packageRules"].as_array();

    rules
        .into_iter()
        .flatten()
        .find(|rule| {
            string_list(rule, "matchManagers") == [manager]
                && string_list(rule, "matchPackageNames") == [STOMME_PACKAGE]
        })
        .unwrap_or_else(|| panic!("no packageRules entry for {manager} and {STOMME_PACKAGE}"))
}

fn custom_manager<'a>(preset: &'a Value, file_pattern: &str) -> &'a Value {
    let managers = preset["customManagers"].as_array();

    managers
        .into_iter()
        .flatten()
        .find(|manager| {
            string_list(manager, "managerFilePatterns") == [file_pattern]
                && manager["depNameTemplate"].as_str() == Some(STOMME_PACKAGE)
        })
        .unwrap_or_else(|| panic!("no custom manager for {file_pattern} and {STOMME_PACKAGE}"))
}
