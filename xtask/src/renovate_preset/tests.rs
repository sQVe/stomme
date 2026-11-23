use super::*;

const DIGEST_AND_VERSION_TEMPLATE: &str =
    "uses: sQVe/stomme/.github/workflows/{{depType}}.yml@{{newDigest}} # {{newValue}}\n";
const SYNC_WORKFLOW: &str = "sync-update";
const OCR_WORKFLOW: &str = "ocr-review";
const SYNC_WORKFLOW_REFERENCE: &str = "sQVe/stomme/.github/workflows/sync-update.yml";
const OCR_WORKFLOW_REFERENCE: &str = "sQVe/stomme/.github/workflows/ocr-review.yml";
const RELEASE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const OLD_RELEASE_COMMIT: &str = "89abcdef0123456789abcdef0123456789abcdef";
const RELEASE_VERSION: &str = "v0.2.3";

const WORKFLOWS: [(&str, &str); 2] = [
    (SYNC_WORKFLOW, SYNC_WORKFLOW_REFERENCE),
    (OCR_WORKFLOW, OCR_WORKFLOW_REFERENCE),
];

fn workflow_manager(preset: &Value) -> &Value {
    custom_manager(preset, WORKFLOW_FILE_PATTERN)
}

fn matching_captures<'a>(manager: &Value, line: &'a str) -> Vec<regex::Captures<'a>> {
    let patterns = string_list(manager, "matchStrings");

    patterns
        .into_iter()
        .map(|pattern| regex::Regex::new(pattern).expect("the match string is a valid regex"))
        .filter_map(|pattern| pattern.captures(line))
        .collect()
}

fn capture<'a>(captures: &regex::Captures<'a>, name: &str) -> Option<&'a str> {
    captures.name(name).map(|capture| capture.as_str())
}

#[derive(Debug, PartialEq, Eq)]
struct Dependency {
    dep_type: String,
    digest: Option<String>,
    value: String,
}

fn dependency(captures: &regex::Captures<'_>) -> Dependency {
    Dependency {
        dep_type: capture(captures, "depType")
            .expect("the match string captures the workflow kind")
            .to_string(),
        digest: capture(captures, "currentDigest").map(ToString::to_string),
        value: capture(captures, "currentValue")
            .expect("the match string captures the current value")
            .to_string(),
    }
}

fn extract_all(manager: &Value, text: &str) -> Vec<Dependency> {
    let mut dependencies = Vec::new();

    for pattern in string_list(manager, "matchStrings") {
        let regex = regex::Regex::new(pattern).expect("the match string is a valid regex");

        dependencies.extend(
            regex
                .captures_iter(text)
                .map(|captures| dependency(&captures)),
        );
    }

    dependencies
}

fn extract(manager: &Value, text: &str) -> Option<Dependency> {
    extract_all(manager, text).into_iter().next()
}

fn replacement(manager: &Value, dependency: &Dependency, digest: &str, value: &str) -> String {
    let template = manager["autoReplaceStringTemplate"]
        .as_str()
        .expect("the manager has an auto-replace template");

    template
        .replace("{{depType}}", &dependency.dep_type)
        .replace("{{newDigest}}", digest)
        .replace("{{newValue}}", value)
}

fn rewrite(manager: &Value, text: &str, new_digest: &str, new_value: &str) -> String {
    for pattern in string_list(manager, "matchStrings") {
        let regex = regex::Regex::new(pattern).expect("the match string is a valid regex");
        let Some(captures) = regex.captures(text) else {
            continue;
        };
        let matched = captures.get(0).expect("a capture has a whole match");
        let replacement = replacement(manager, &dependency(&captures), new_digest, new_value);

        return format!(
            "{}{replacement}{}",
            &text[..matched.start()],
            &text[matched.end()..]
        );
    }

    text.to_string()
}

fn rewrite_all(manager: &Value, text: &str, new_digest: &str, new_value: &str) -> String {
    let mut rewritten = String::new();
    let mut position = 0;

    for pattern in string_list(manager, "matchStrings") {
        let regex = regex::Regex::new(pattern).expect("the match string is a valid regex");

        for captures in regex.captures_iter(text) {
            let matched = captures.get(0).expect("a capture has a whole match");
            let replacement = replacement(manager, &dependency(&captures), new_digest, new_value);

            rewritten.push_str(&text[position..matched.start()]);
            rewritten.push_str(&replacement);
            position = matched.end();
        }
    }

    rewritten.push_str(&text[position..]);

    rewritten
}

fn workflow_reference(reference: &str) -> String {
    format!("uses: {reference}@{RELEASE_COMMIT} # {RELEASE_VERSION}")
}

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
fn preset_holds_one_workflow_manager() {
    let preset = renovate_preset();
    let managers = preset["customManagers"]
        .as_array()
        .expect("the preset has custom managers");
    let workflow_managers: Vec<&Value> = managers
        .iter()
        .filter(|manager| {
            string_list(manager, "managerFilePatterns") == [WORKFLOW_FILE_PATTERN]
                && manager["depNameTemplate"].as_str() == Some(STOMME_PACKAGE)
        })
        .collect();

    assert_eq!(workflow_managers.len(), 1);
}

#[test]
fn workflow_manager_keeps_the_v_prefix() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset);

    assert!(
        manager.get("extractVersionTemplate").is_none(),
        "the v prefix names the release tag, so the manager must not strip it"
    );
}

#[test]
fn workflow_manager_writes_the_digest_and_version_comment() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset);

    assert_eq!(
        manager["autoReplaceStringTemplate"].as_str(),
        Some(DIGEST_AND_VERSION_TEMPLATE)
    );
}

#[test]
fn workflow_manager_reads_the_tag_and_digest_forms() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset);
    let cases: [(&str, String, Option<&str>, &str); 4] = [
        (
            SYNC_WORKFLOW,
            format!("    uses: {SYNC_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # v0.2.3\n"),
            Some(RELEASE_COMMIT),
            "v0.2.3",
        ),
        (
            SYNC_WORKFLOW,
            format!("    uses: {SYNC_WORKFLOW_REFERENCE}@v0.2.0\n"),
            None,
            "v0.2.0",
        ),
        (
            OCR_WORKFLOW,
            format!("    uses: {OCR_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # v0.2.3\n"),
            Some(RELEASE_COMMIT),
            "v0.2.3",
        ),
        (
            OCR_WORKFLOW,
            format!("    uses: {OCR_WORKFLOW_REFERENCE}@v0.2.0\n"),
            None,
            "v0.2.0",
        ),
    ];

    for (dep_type, line, digest, value) in cases {
        let matches = matching_captures(manager, &line);

        assert_eq!(matches.len(), 1, "{line}");
        assert_eq!(capture(&matches[0], "depType"), Some(dep_type), "{line}");
        assert_eq!(capture(&matches[0], "currentDigest"), digest, "{line}");
        assert_eq!(capture(&matches[0], "currentValue"), Some(value), "{line}");
    }
}

#[test]
fn workflow_manager_rewrites_the_tag_and_digest_forms() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset);

    for (dep_type, reference) in WORKFLOWS {
        let cases = [
            format!("    uses: {reference}@v0.2.0\n"),
            format!("    uses: {reference}@{OLD_RELEASE_COMMIT} # v0.2.0\n"),
        ];

        for line in cases {
            let text = format!("jobs:\n  review:\n{line}");
            let rewritten = rewrite(manager, &text, RELEASE_COMMIT, RELEASE_VERSION);
            let expected = format!("jobs:\n  review:\n    {}\n", workflow_reference(reference));

            assert_eq!(rewritten, expected, "{line}");
            assert_eq!(
                extract_all(manager, &rewritten),
                [Dependency {
                    dep_type: dep_type.to_string(),
                    digest: Some(RELEASE_COMMIT.to_string()),
                    value: RELEASE_VERSION.to_string(),
                }],
                "{line}"
            );
        }
    }
}

#[test]
fn workflow_manager_leaves_quoted_and_run_calls_alone() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset);

    for (_, reference) in WORKFLOWS {
        let cases = [
            format!("    uses: \"{reference}@v0.2.0\"\n"),
            format!("    uses: '{reference}@v0.2.0'\n"),
            format!("    uses: \"{reference}@{OLD_RELEASE_COMMIT} # v0.2.0\"\n"),
            format!("    uses: '{reference}@{OLD_RELEASE_COMMIT} # v0.2.0'\n"),
            format!(
                "    steps:\n      - run: |\n          test -n \"$REPORT\" && echo \"uses: {reference}@v0.2.0\"\n"
            ),
            format!(
                "    steps:\n      - run: |\n          test -n \"$REPORT\" && echo \"uses: {reference}@{OLD_RELEASE_COMMIT} # v0.2.0\"\n"
            ),
        ];

        for text in cases {
            assert_eq!(matching_captures(manager, &text).len(), 0, "{text}");
            assert_eq!(extract(manager, &text), None, "{text}");
            assert_eq!(
                rewrite(manager, &text, RELEASE_COMMIT, RELEASE_VERSION),
                text,
                "{text}"
            );
        }
    }
}

#[test]
fn workflow_manager_rewrites_both_workflows_in_one_file() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset);
    let text = format!(
        "jobs:\n  sync:\n    uses: {SYNC_WORKFLOW_REFERENCE}@v0.2.0\n  review:\n    uses: {OCR_WORKFLOW_REFERENCE}@v0.2.0\n"
    );

    assert_ne!(
        manager["matchStringsStrategy"].as_str(),
        Some("combination"),
        "the combination strategy merges every call into one dependency"
    );
    assert_eq!(
        extract_all(manager, &text),
        [
            Dependency {
                dep_type: SYNC_WORKFLOW.to_string(),
                digest: None,
                value: "v0.2.0".to_string(),
            },
            Dependency {
                dep_type: OCR_WORKFLOW.to_string(),
                digest: None,
                value: "v0.2.0".to_string(),
            },
        ]
    );

    let rewritten = rewrite_all(manager, &text, RELEASE_COMMIT, RELEASE_VERSION);
    let expected = format!(
        "jobs:\n  sync:\n    {}\n  review:\n    {}\n",
        workflow_reference(SYNC_WORKFLOW_REFERENCE),
        workflow_reference(OCR_WORKFLOW_REFERENCE),
    );

    assert_eq!(rewritten, expected);
    assert_eq!(
        extract_all(manager, &rewritten),
        [
            Dependency {
                dep_type: SYNC_WORKFLOW.to_string(),
                digest: Some(RELEASE_COMMIT.to_string()),
                value: RELEASE_VERSION.to_string(),
            },
            Dependency {
                dep_type: OCR_WORKFLOW.to_string(),
                digest: Some(RELEASE_COMMIT.to_string()),
                value: RELEASE_VERSION.to_string(),
            },
        ]
    );
}

#[test]
fn sync_workflow_pin_rule_pins_digests_for_workflows_only() {
    let preset = renovate_preset();
    let rule = package_rule(&preset, "custom.regex", &[]);

    assert_eq!(rule["pinDigests"].as_bool(), Some(true));
    assert_eq!(
        string_list(rule, "matchFileNames"),
        [".github/workflows/**"]
    );
}

#[test]
fn github_actions_manager_is_disabled_for_stomme() {
    let preset = renovate_preset();
    let rule = package_rule(&preset, "github-actions", &[]);

    assert_eq!(rule["enabled"].as_bool(), Some(false));
}

#[test]
fn stomme_updates_group_under_one_name() {
    let preset = renovate_preset();
    let rules = preset["packageRules"]
        .as_array()
        .expect("the preset has package rules");
    let group = rules
        .iter()
        .find(|rule| rule["groupSlug"].as_str() == Some("stomme"))
        .expect("the preset groups stomme updates");

    assert_eq!(string_list(group, "matchPackageNames"), [STOMME_PACKAGE]);
    assert_eq!(group["groupName"].as_str(), Some("stomme"));
    assert_eq!(group["groupSingleUpdates"].as_bool(), Some(true));
    assert_eq!(group["separateMajorMinor"].as_bool(), Some(false));
}
