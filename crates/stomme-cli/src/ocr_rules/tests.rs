use std::fs;
use std::path::Path;

use super::*;

#[test]
fn render_without_project_rules_writes_one_language_entry() {
    let guides = [
        ("docs/stomme/shared/coding.md", "Shared rules.\n"),
        ("docs/stomme/languages/rust/guide.md", "Rust rules.\n"),
    ];

    let rendered = render(Language::Rust, &guides, None);
    let document = parse(&rendered);
    let rules = document["rules"].as_array().expect("the rules are a list");

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["path"], "**/*.rs");
    assert_eq!(rules[0]["merge_system_rule"], serde_json::Value::Bool(true));

    let text = rules[0]["rule"].as_str().expect("the rule is text");
    let shared = text
        .find("File: docs/stomme/shared/coding.md")
        .expect("the shared guide is named");
    let rust = text
        .find("File: docs/stomme/languages/rust/guide.md")
        .expect("the Rust guide is named");

    assert!(shared < rust);
    assert!(document.get("include").is_none());
    assert!(document.get("exclude").is_none());
}

#[test]
fn render_puts_project_rules_first_with_guide_text() {
    let root = project(
        r#"{
            "rules": [
                { "path": "src/**", "rule": "Keep modules small." },
                { "path": "tests/**", "rule": "Name the behavior." }
            ],
            "exclude": ["fixtures/**"]
        }"#,
    );
    let rules = read_rules(root.path());
    let guides = [("docs/stomme/shared/coding.md", "Shared rules.\n")];

    let rendered = render(Language::TypeScript, &guides, Some(&rules));
    let document = parse(&rendered);
    let entries = document["rules"].as_array().expect("the rules are a list");

    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["path"], "src/**");
    assert_eq!(entries[1]["path"], "tests/**");
    assert_eq!(entries[2]["path"], "**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}");

    let first = entries[0]["rule"].as_str().expect("the rule is text");
    let second = entries[1]["rule"].as_str().expect("the rule is text");

    assert!(first.starts_with("Keep modules small."));
    assert!(first.contains("File: docs/stomme/shared/coding.md"));
    assert!(first.contains("Shared rules."));
    assert!(second.starts_with("Name the behavior."));
    assert!(second.contains("File: docs/stomme/shared/coding.md"));
    assert_eq!(document["exclude"], serde_json::json!(["fixtures/**"]));
}

#[test]
fn read_project_without_file_returns_none() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    assert!(
        read_project(root.path())
            .expect("read the project rules")
            .is_none()
    );

    fs::create_dir(root.path().join(".opencodereview")).expect("create the rule folder");

    assert!(
        read_project(root.path())
            .expect("read the project rules")
            .is_none()
    );
}

#[test]
fn read_project_refuses_invalid_json() {
    let root = project("{");

    let error = read_project(root.path()).expect_err("refuse invalid JSON");

    assert!(matches!(
        error,
        OcrRulesError::InvalidJson { path, .. } if path == PROJECT_RULE_FILE
    ));
}

#[test]
fn read_project_refuses_invalid_utf8() {
    let mut bytes = br#"{"rules":[{"path":"src/**","rule":""#.to_vec();

    bytes.push(0xff);
    bytes.extend_from_slice(br#""}]}"#);

    let root = project_bytes(&bytes);
    let error = read_project(root.path()).expect_err("refuse invalid UTF-8");

    assert!(matches!(
        error,
        OcrRulesError::InvalidJson { path, .. } if path == PROJECT_RULE_FILE
    ));
}

#[test]
fn read_project_refuses_a_root_array() {
    let root = project("[[]]");

    let error = read_project(root.path()).expect_err("refuse an array root");

    assert!(matches!(
        error,
        OcrRulesError::Shape { path, reason }
            if path == PROJECT_RULE_FILE && reason == "expected an object"
    ));
}

#[test]
fn read_project_refuses_null_rules() {
    let root = project(r#"{"rules":null}"#);

    let error = read_project(root.path()).expect_err("refuse null rules");

    assert!(matches!(
        error,
        OcrRulesError::Shape { path, reason }
            if path == PROJECT_RULE_FILE && reason.contains("null")
    ));
}

#[test]
fn read_project_refuses_unknown_key() {
    let root = project(r#"{"rules":[],"model":"x"}"#);

    let error = read_project(root.path()).expect_err("refuse an unknown key");

    assert!(matches!(
        error,
        OcrRulesError::Shape { path, reason }
            if path == PROJECT_RULE_FILE && reason.contains("model")
    ));
}

#[test]
fn read_project_refuses_empty_path() {
    let root = project(r#"{"rules":[{"path":"src/**","rule":"x"},{"path":" ","rule":"y"}]}"#);

    let error = read_project(root.path()).expect_err("refuse an empty path");

    assert!(matches!(
        error,
        OcrRulesError::EmptyPath { path, index } if path == PROJECT_RULE_FILE && index == 2
    ));
}

#[test]
fn read_project_refuses_empty_rule() {
    let root = project(r#"{"rules":[{"path":"src/**","rule":""}]}"#);

    let error = read_project(root.path()).expect_err("refuse an empty rule");

    assert!(matches!(
        error,
        OcrRulesError::EmptyRule { path, index } if path == PROJECT_RULE_FILE && index == 1
    ));
}

#[test]
fn read_project_refuses_a_whitespace_path() {
    let root = project(r#"{"rules":[{"path":"  \t","rule":"x"}]}"#);

    let error = read_project(root.path()).expect_err("refuse a whitespace path");

    assert!(matches!(
        error,
        OcrRulesError::EmptyPath { path, index } if path == PROJECT_RULE_FILE && index == 1
    ));
}

#[test]
fn read_project_refuses_a_whitespace_rule() {
    let root = project(r#"{"rules":[{"path":"src/**","rule":" \n\t"}]}"#);

    let error = read_project(root.path()).expect_err("refuse a whitespace rule");

    assert!(matches!(
        error,
        OcrRulesError::EmptyRule { path, index } if path == PROJECT_RULE_FILE && index == 1
    ));
}

#[cfg(unix)]
#[test]
fn read_project_refuses_a_symlinked_file() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("create a temporary directory");
    let outside = tempfile::tempdir().expect("create an outside directory");
    let target = outside.path().join("rule.json");

    fs::write(&target, r#"{"rules":[]}"#).expect("write the outside rule file");
    fs::create_dir(root.path().join(".opencodereview")).expect("create the rule folder");
    symlink(&target, root.path().join(PROJECT_RULE_FILE)).expect("create the symlink");

    let error = read_project(root.path()).expect_err("refuse a symlinked rule file");

    assert!(matches!(
        error,
        OcrRulesError::NotAFile { path } if path == PROJECT_RULE_FILE
    ));
}

#[cfg(unix)]
#[test]
fn read_project_refuses_a_symlinked_folder() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("create a temporary directory");
    let outside = tempfile::tempdir().expect("create an outside directory");

    fs::write(
        outside.path().join("rule.json"),
        r#"{"rules":[{"path":"src/**","rule":"Outside marker"}]}"#,
    )
    .expect("write the outside rule file");
    symlink(outside.path(), root.path().join(".opencodereview")).expect("create the symlink");

    let error = read_project(root.path()).expect_err("refuse a symlinked rule folder");

    assert!(matches!(
        error,
        OcrRulesError::NotAFile { path } if path == PROJECT_RULE_FILE
    ));
}

#[test]
fn read_project_refuses_positional_rules() {
    let root = project(r#"{"rules":[["src/**","x"]]}"#);

    let error = read_project(root.path()).expect_err("refuse positional rules");

    assert!(matches!(
        error,
        OcrRulesError::Shape { path, reason }
            if path == PROJECT_RULE_FILE && reason == "expected an object"
    ));
}

#[test]
fn read_project_accepts_a_filter_only_file() {
    let root = project(r#"{"exclude":["fixtures/**"]}"#);

    let rules = read_rules(root.path());

    assert!(rules.rules.is_empty());
    assert_eq!(rules.exclude, ["fixtures/**"]);
}

#[test]
fn render_ends_with_a_newline_and_keeps_key_order() {
    let guides = [("docs/stomme/shared/coding.md", "Shared rules.")];

    let rendered = render(Language::Rust, &guides, None);

    assert!(rendered.ends_with('\n'));

    let path = rendered.find("\"path\"").expect("an entry has a path");
    let rule = rendered.find("\"rule\"").expect("an entry has a rule");
    let merge = rendered
        .find("\"merge_system_rule\"")
        .expect("an entry merges the system rule");

    assert!(path < rule);
    assert!(rule < merge);
}

#[test]
fn render_writes_include_only_when_it_is_not_empty() {
    let root = project(r#"{"include":["src/**"],"rules":[]}"#);
    let rules = read_rules(root.path());

    let rendered = render(Language::Rust, &[], Some(&rules));
    let document = parse(&rendered);

    assert_eq!(document["include"], serde_json::json!(["src/**"]));
    assert!(document.get("exclude").is_none());
}

#[test]
fn render_appends_the_guide_text_to_a_project_rule() {
    let root = project(
        r#"{
            "rules": [
                { "path": "src/**", "rule": "Keep modules small." },
                { "path": "tests/**", "rule": "Name the behavior." }
            ],
            "include": ["src/**"],
            "exclude": ["fixtures/**"]
        }"#,
    );
    let rules = read_rules(root.path());
    let guides = [("docs/stomme/shared/coding.md", "Shared rules.\n")];
    let guide_text = r#"Apply stomme's coding guides below. Each guide starts with its repository path. Resolve its relative links from that path, and read linked files with file_read.\n\nFile: docs/stomme/shared/coding.md\n\nShared rules.\n\nAlso apply the rules in the project's AGENTS.md and the decisions in docs/adr/ that cover the change. Read them with file_read."#;
    let expected = format!(
        r#"{{
  "rules": [
    {{
      "path": "src/**",
      "rule": "Keep modules small.\n\n{guide_text}",
      "merge_system_rule": true
    }},
    {{
      "path": "tests/**",
      "rule": "Name the behavior.\n\n{guide_text}",
      "merge_system_rule": true
    }},
    {{
      "path": "**/*.rs",
      "rule": "{guide_text}",
      "merge_system_rule": true
    }}
  ],
  "include": [
    "src/**"
  ],
  "exclude": [
    "fixtures/**"
  ]
}}
"#
    );

    let rendered = render(Language::Rust, &guides, Some(&rules));

    assert_eq!(rendered, expected);
}

fn parse(rendered: &str) -> serde_json::Value {
    serde_json::from_str(rendered).expect("the rendered text is JSON")
}

fn project(text: &str) -> tempfile::TempDir {
    project_bytes(text.as_bytes())
}

fn project_bytes(bytes: &[u8]) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let path = root.path().join(PROJECT_RULE_FILE);
    let folder = path
        .parent()
        .expect("the rule file lies inside the project");

    fs::create_dir_all(folder).expect("create the rule folder");
    fs::write(&path, bytes).expect("write the rule file");

    root
}

fn read_rules(root: &Path) -> ProjectRules {
    read_project(root)
        .expect("read the project rules")
        .expect("the project has a rule file")
}
