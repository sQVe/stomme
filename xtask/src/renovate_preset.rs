use serde_json::Value;

const RENOVATE_PRESET: &str = include_str!("../../renovate/stomme.json");
const STOMME_PACKAGE: &str = "sQVe/stomme";
const MISE_DEPENDENCY: &str = "github:sQVe/stomme";
const BARE_VERSION_EXTRACTION: &str = "^v(?<version>.*)$";
const STOMME_TOML_FILE_PATTERN: &str = r"/^stomme\.toml$/";
const WORKFLOW_FILE_PATTERN: &str = r"/^\.github\/workflows\/[^/]+\.ya?ml$/";

fn renovate_preset() -> Value {
    serde_json::from_str(RENOVATE_PRESET).expect("the Renovate preset is valid JSON")
}

fn string_list<'a>(value: &'a Value, key: &str) -> Vec<&'a str> {
    value[key]
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn package_rule<'a>(preset: &'a Value, manager: &str, dependencies: &[&str]) -> &'a Value {
    let rules = preset["packageRules"].as_array();

    rules
        .into_iter()
        .flatten()
        .find(|rule| {
            string_list(rule, "matchManagers") == [manager]
                && string_list(rule, "matchDepNames") == dependencies
                && string_list(rule, "matchPackageNames") == [STOMME_PACKAGE]
        })
        .unwrap_or_else(|| {
            panic!("no packageRules entry for {manager}, {dependencies:?} and {STOMME_PACKAGE}")
        })
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

fn workflow_manager<'a>(preset: &'a Value, workflow: &str) -> &'a Value {
    let reference = format!("/{workflow}");
    let managers = preset["customManagers"].as_array();

    managers
        .into_iter()
        .flatten()
        .find(|manager| {
            string_list(manager, "managerFilePatterns") == [WORKFLOW_FILE_PATTERN]
                && manager["depNameTemplate"].as_str() == Some(STOMME_PACKAGE)
                && string_list(manager, "matchStrings")
                    .iter()
                    .all(|pattern| pattern.contains(&reference))
        })
        .unwrap_or_else(|| panic!("no workflow manager for {workflow}"))
}

#[cfg(test)]
mod tests;
