use super::*;

#[test]
fn mise_rule_extracts_bare_versions_for_the_github_backend() {
    let preset = renovate_preset();
    let rule = package_rule(&preset, "mise", &[MISE_DEPENDENCY]);

    assert_eq!(
        rule["extractVersion"].as_str(),
        Some(BARE_VERSION_EXTRACTION)
    );
}

#[test]
fn stomme_toml_manager_extracts_bare_versions() {
    let preset = renovate_preset();
    let manager = custom_manager(&preset, STOMME_TOML_FILE_PATTERN);

    assert_eq!(
        manager["extractVersionTemplate"].as_str(),
        Some(BARE_VERSION_EXTRACTION)
    );
}

#[test]
fn sync_workflow_manager_extracts_bare_versions() {
    let preset = renovate_preset();
    let manager = custom_manager(&preset, WORKFLOW_FILE_PATTERN);

    assert_eq!(
        manager["extractVersionTemplate"].as_str(),
        Some(BARE_VERSION_EXTRACTION)
    );
}

#[test]
fn sync_workflow_manager_writes_the_v_tag_prefix() {
    let preset = renovate_preset();
    let manager = custom_manager(&preset, WORKFLOW_FILE_PATTERN);
    let template = manager["autoReplaceStringTemplate"].as_str();

    assert!(
        template.is_some_and(|template| template.ends_with("@v{{newValue}}")),
        "template: {template:?}"
    );
}

#[test]
fn github_actions_manager_is_disabled_for_stomme() {
    let preset = renovate_preset();
    let rule = package_rule(&preset, "github-actions", &[]);

    assert_eq!(rule["enabled"].as_bool(), Some(false));
}
