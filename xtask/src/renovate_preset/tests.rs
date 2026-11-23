use super::*;

const DIGEST_AND_VERSION_TEMPLATE: &str =
    "uses: sQVe/stomme/.github/workflows/sync-update.yml@{{newDigest}} # {{newValue}}\n";
const OCR_DIGEST_AND_VERSION_TEMPLATE: &str =
    "uses: sQVe/stomme/.github/workflows/ocr-review.yml@{{newDigest}} # {{newValue}}\n";
const SYNC_WORKFLOW: &str = "sync-update";
const OCR_WORKFLOW: &str = "ocr-review";
const SYNC_WORKFLOW_REFERENCE: &str = "sQVe/stomme/.github/workflows/sync-update.yml";
const OCR_WORKFLOW_REFERENCE: &str = "sQVe/stomme/.github/workflows/ocr-review.yml";
const RELEASE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const OLD_RELEASE_COMMIT: &str = "89abcdef0123456789abcdef0123456789abcdef";
const RELEASE_VERSION: &str = "v0.2.3";

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
    digest: Option<String>,
    value: String,
}

fn extract(manager: &Value, text: &str) -> Option<Dependency> {
    for pattern in string_list(manager, "matchStrings") {
        let regex = regex::Regex::new(pattern).expect("the match string is a valid regex");
        let Some(captures) = regex.captures(text) else {
            continue;
        };

        return Some(Dependency {
            digest: capture(&captures, "currentDigest").map(ToString::to_string),
            value: capture(&captures, "currentValue")
                .expect("the match string captures the current value")
                .to_string(),
        });
    }

    None
}

fn rewrite(manager: &Value, text: &str, new_digest: &str, new_value: &str) -> String {
    let template = manager["autoReplaceStringTemplate"]
        .as_str()
        .expect("the manager has an auto-replace template");
    let replacement = template
        .replace("{{newDigest}}", new_digest)
        .replace("{{newValue}}", new_value);

    for pattern in string_list(manager, "matchStrings") {
        let regex = regex::Regex::new(pattern).expect("the match string is a valid regex");
        let Some(matched) = regex.find(text) else {
            continue;
        };

        return format!(
            "{}{replacement}{}",
            &text[..matched.start()],
            &text[matched.end()..]
        );
    }

    text.to_string()
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
fn sync_workflow_manager_keeps_the_v_prefix() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, SYNC_WORKFLOW);

    assert!(
        manager.get("extractVersionTemplate").is_none(),
        "the v prefix names the release tag, so the manager must not strip it"
    );
}

#[test]
fn sync_workflow_manager_writes_the_digest_and_version_comment() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, SYNC_WORKFLOW);

    assert_eq!(
        manager["autoReplaceStringTemplate"].as_str(),
        Some(DIGEST_AND_VERSION_TEMPLATE)
    );
}

#[test]
fn sync_workflow_manager_reads_the_digest_form() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, SYNC_WORKFLOW);
    let line = format!("    uses: {SYNC_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # v0.2.3\n");

    let matches = matching_captures(manager, &line);

    assert_eq!(matches.len(), 1, "one match string reads the digest form");
    assert_eq!(capture(&matches[0], "currentDigest"), Some(RELEASE_COMMIT));
    assert_eq!(capture(&matches[0], "currentValue"), Some("v0.2.3"));
}

#[test]
fn sync_workflow_manager_reads_the_tag_form() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, SYNC_WORKFLOW);
    let line = format!("    uses: {SYNC_WORKFLOW_REFERENCE}@v0.2.0\n");

    let matches = matching_captures(manager, &line);

    assert_eq!(matches.len(), 1, "one match string reads the tag form");
    assert_eq!(capture(&matches[0], "currentDigest"), None);
    assert_eq!(capture(&matches[0], "currentValue"), Some("v0.2.0"));
}

#[test]
fn sync_workflow_manager_rewrites_the_unquoted_tag_form() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, SYNC_WORKFLOW);
    let text = format!("jobs:\n  sync:\n    uses: {SYNC_WORKFLOW_REFERENCE}@v0.2.0\n");

    let before = extract(manager, &text).expect("the tag form extracts");

    assert_eq!(
        before,
        Dependency {
            digest: None,
            value: "v0.2.0".to_string(),
        }
    );

    let rewritten = rewrite(manager, &text, RELEASE_COMMIT, RELEASE_VERSION);
    let expected = format!(
        "jobs:\n  sync:\n    uses: {SYNC_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # {RELEASE_VERSION}\n"
    );

    assert_eq!(rewritten, expected);

    let after = extract(manager, &rewritten).expect("the rewritten form extracts");

    assert_eq!(
        after,
        Dependency {
            digest: Some(RELEASE_COMMIT.to_string()),
            value: RELEASE_VERSION.to_string(),
        }
    );
}

#[test]
fn sync_workflow_manager_rewrites_the_unquoted_digest_form() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, SYNC_WORKFLOW);
    let text = format!(
        "jobs:\n  sync:\n    uses: {SYNC_WORKFLOW_REFERENCE}@{OLD_RELEASE_COMMIT} # v0.2.0\n"
    );

    let before = extract(manager, &text).expect("the digest form extracts");

    assert_eq!(
        before,
        Dependency {
            digest: Some(OLD_RELEASE_COMMIT.to_string()),
            value: "v0.2.0".to_string(),
        }
    );

    let rewritten = rewrite(manager, &text, RELEASE_COMMIT, RELEASE_VERSION);
    let expected = format!(
        "jobs:\n  sync:\n    uses: {SYNC_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # {RELEASE_VERSION}\n"
    );

    assert_eq!(rewritten, expected);

    let after = extract(manager, &rewritten).expect("the rewritten form extracts");

    assert_eq!(
        after,
        Dependency {
            digest: Some(RELEASE_COMMIT.to_string()),
            value: RELEASE_VERSION.to_string(),
        }
    );
}

#[test]
fn sync_workflow_manager_ignores_uses_inside_a_run_value() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, SYNC_WORKFLOW);
    let cases = [
        format!(
            "jobs:\n  sync:\n    steps:\n      - run: |\n          test -n \"$REPORT\" && echo \"uses: {SYNC_WORKFLOW_REFERENCE}@v0.2.0\"\n"
        ),
        format!(
            "jobs:\n  sync:\n    steps:\n      - run: |\n          test -n \"$REPORT\" && echo \"uses: {SYNC_WORKFLOW_REFERENCE}@{OLD_RELEASE_COMMIT} # v0.2.0\"\n"
        ),
    ];

    for text in cases {
        assert_eq!(extract(manager, &text), None, "{text}");
        assert_eq!(
            rewrite(manager, &text, RELEASE_COMMIT, RELEASE_VERSION),
            text,
            "{text}"
        );
    }
}

#[test]
fn sync_workflow_manager_leaves_quoted_uses_calls_alone() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, SYNC_WORKFLOW);
    let cases = [
        format!("    uses: \"{SYNC_WORKFLOW_REFERENCE}@v0.2.0\""),
        format!("    uses: '{SYNC_WORKFLOW_REFERENCE}@v0.2.0'"),
        format!("    uses: \"{SYNC_WORKFLOW_REFERENCE}@{OLD_RELEASE_COMMIT} # v0.2.0\""),
        format!("    uses: '{SYNC_WORKFLOW_REFERENCE}@{OLD_RELEASE_COMMIT} # v0.2.0'"),
    ];

    for line in cases {
        let text = format!("jobs:\n  sync:\n{line}\n");

        assert_eq!(extract(manager, &text), None, "{line}");
        assert_eq!(
            rewrite(manager, &text, RELEASE_COMMIT, RELEASE_VERSION),
            text,
            "{line}"
        );
    }
}

#[test]
fn ocr_workflow_manager_handles_each_call_form() {
    let preset = renovate_preset();
    let manager = workflow_manager(&preset, OCR_WORKFLOW);

    assert!(
        manager.get("extractVersionTemplate").is_none(),
        "the v prefix names the release tag, so the manager must not strip it"
    );
    assert_eq!(
        manager["autoReplaceStringTemplate"].as_str(),
        Some(OCR_DIGEST_AND_VERSION_TEMPLATE)
    );

    let jobs = "jobs:\n  review:\n";
    let digest_form =
        format!("{jobs}    uses: {OCR_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # v0.2.3\n");
    let old_digest_form =
        format!("{jobs}    uses: {OCR_WORKFLOW_REFERENCE}@{OLD_RELEASE_COMMIT} # v0.2.3\n");
    let tag_form = format!("{jobs}    uses: {OCR_WORKFLOW_REFERENCE}@v0.2.0\n");
    let rewritten =
        format!("{jobs}    uses: {OCR_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # {RELEASE_VERSION}\n");
    let run_digest = format!(
        "{jobs}    steps:\n      - run: |\n          test -n \"$REPORT\" && echo \"uses: {OCR_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # v0.2.3\"\n"
    );
    let run_tag = format!(
        "{jobs}    steps:\n      - run: |\n          test -n \"$REPORT\" && echo \"uses: {OCR_WORKFLOW_REFERENCE}@v0.2.0\"\n"
    );
    let quoted_digest =
        format!("{jobs}    uses: \"{OCR_WORKFLOW_REFERENCE}@{RELEASE_COMMIT} # v0.2.3\"\n");
    let quoted_tag = format!("{jobs}    uses: '{OCR_WORKFLOW_REFERENCE}@v0.2.0'\n");
    let cases = [
        (
            "the digest form",
            digest_form,
            1,
            Some(Dependency {
                digest: Some(RELEASE_COMMIT.to_string()),
                value: "v0.2.3".to_string(),
            }),
            rewritten.clone(),
        ),
        (
            "the tag form",
            tag_form,
            1,
            Some(Dependency {
                digest: None,
                value: "v0.2.0".to_string(),
            }),
            rewritten.clone(),
        ),
        (
            "the old digest form",
            old_digest_form,
            1,
            Some(Dependency {
                digest: Some(OLD_RELEASE_COMMIT.to_string()),
                value: "v0.2.3".to_string(),
            }),
            rewritten,
        ),
        ("a run value", run_digest.clone(), 0, None, run_digest),
        ("a run value with a tag", run_tag.clone(), 0, None, run_tag),
        (
            "a quoted digest form",
            quoted_digest.clone(),
            0,
            None,
            quoted_digest,
        ),
        ("a quoted tag form", quoted_tag.clone(), 0, None, quoted_tag),
    ];

    for (name, text, captures, before, after) in cases {
        assert_eq!(
            matching_captures(manager, &text).len(),
            captures,
            "{name}: {text}"
        );
        assert_eq!(extract(manager, &text), before, "{name}: {text}");
        assert_eq!(
            rewrite(manager, &text, RELEASE_COMMIT, RELEASE_VERSION),
            after,
            "{name}: {text}"
        );
    }
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
