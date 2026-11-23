use std::fs;
use std::path::Path;

use super::*;

#[test]
fn render_without_project_rules_writes_one_language_entry() {
    let rendered = render(Language::Rust, &guides(), None);
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
    assert_eq!(
        document["include"],
        serde_json::json!([
            "**/*_test.rs",
            "**/*_tests.rs",
            "**/tests.rs",
            "**/tests/**/*.rs",
            "**/benches/**/*.rs",
        ])
    );
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

    let rendered = render(Language::TypeScript, &guides(), Some(&rules));
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
fn read_project_refuses_a_duplicate_member() {
    let root = project(r#"{"rules":[{"path":" ","path":"src/**","rule":"x"}]}"#);

    let error = read_project(root.path()).expect_err("refuse a duplicate path");

    assert!(matches!(
        error,
        OcrRulesError::Shape { path, reason }
            if path == PROJECT_RULE_FILE && reason.contains("duplicate field `path`")
    ));
}

#[test]
fn read_project_refuses_a_nested_merge_system_rule() {
    let root = project(r#"{"rules":[{"path":"src/**","rule":"x","merge_system_rule":true}]}"#);

    let error = read_project(root.path()).expect_err("refuse a nested merge flag");

    assert!(matches!(
        error,
        OcrRulesError::Shape { path, reason }
            if path == PROJECT_RULE_FILE && reason.contains("merge_system_rule")
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
fn read_project_refuses_an_unreadable_rule_file() {
    use std::os::unix::fs::PermissionsExt;

    struct RestorePermissions<'a> {
        path: &'a Path,
        mode: u32,
    }

    impl Drop for RestorePermissions<'_> {
        #[expect(
            clippy::print_stderr,
            reason = "a cleanup error has no caller that can take it"
        )]
        fn drop(&mut self) {
            if let Err(error) =
                fs::set_permissions(self.path, fs::Permissions::from_mode(self.mode))
            {
                eprintln!(
                    "could not restore the permissions of {}: {error}",
                    self.path.display()
                );
            }
        }
    }

    let root = project(r#"{"rules":[]}"#);
    let path = root.path().join(PROJECT_RULE_FILE);
    let mode = fs::metadata(&path)
        .expect("read the rule file metadata")
        .permissions()
        .mode()
        & 0o7777;
    let restore = RestorePermissions { path: &path, mode };

    fs::set_permissions(&path, fs::Permissions::from_mode(0o000))
        .expect("remove the rule file permissions");

    let result = read_project(root.path());

    drop(restore);

    let error = result.expect_err("refuse an unreadable rule file");

    assert!(matches!(
        error,
        OcrRulesError::Read { path, .. } if path == PROJECT_RULE_FILE
    ));
}

#[test]
fn read_project_refuses_a_root_that_is_not_a_folder() {
    let parent = tempfile::tempdir().expect("create a temporary directory");
    let root = parent.path().join("project");

    fs::write(&root, "").expect("write the project root file");

    let error = read_project(&root).expect_err("refuse a root that is not a folder");

    assert!(matches!(
        error,
        OcrRulesError::Read { path, .. } if path == PROJECT_RULE_FILE
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
fn read_project_refuses_an_unknown_language() {
    let root = project(r#"{"rules":[{"path":"packages/**","rule":"x","language":"python"}]}"#);

    let error = read_project(root.path()).expect_err("refuse an unknown language");

    assert!(matches!(
        error,
        OcrRulesError::UnknownLanguage { path, index, language }
            if path == PROJECT_RULE_FILE && index == 1 && language == "python"
    ));
}

#[test]
fn render_ends_with_a_newline_and_keeps_key_order() {
    let rendered = render(Language::Rust, &guides(), None);

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
fn render_keeps_a_project_include_once_before_the_language_patterns() {
    let root = project(r#"{"include":["src/**","**/tests.rs","src/**"],"rules":[]}"#);
    let rules = read_rules(root.path());

    let rendered = render(Language::Rust, &guides(), Some(&rules));
    let document = parse(&rendered);

    assert_eq!(
        document["include"],
        serde_json::json!([
            "src/**",
            "**/tests.rs",
            "**/*_test.rs",
            "**/*_tests.rs",
            "**/tests/**/*.rs",
            "**/benches/**/*.rs",
        ])
    );
    assert!(document.get("exclude").is_none());
}

#[test]
fn render_lists_the_typescript_test_patterns() {
    let rendered = render(Language::TypeScript, &guides(), None);
    let document = parse(&rendered);

    assert_eq!(
        document["include"],
        serde_json::json!([
            "**/*.test.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
            "**/*.spec.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
            "**/__tests__/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
        ])
    );
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
    let guide_text = r#"Apply stomme's coding guides below. Each guide starts with its repository path. Resolve its relative links from that path, and read linked files with file_read.\n\nFile: docs/stomme/shared/coding.md\n\nShared rules.\n\nFile: docs/stomme/languages/rust/guide.md\n\nRust rules.\n\nAlso apply the rules in the project's AGENTS.md and the decisions in docs/adr/ that cover the change. Read them with file_read."#;
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
    "src/**",
    "**/*_test.rs",
    "**/*_tests.rs",
    "**/tests.rs",
    "**/tests/**/*.rs",
    "**/benches/**/*.rs"
  ],
  "exclude": [
    "fixtures/**"
  ]
}}
"#
    );

    let rendered = render(Language::Rust, &guides(), Some(&rules));

    assert_eq!(rendered, expected);
}

#[test]
fn read_project_refuses_a_null_language() {
    let root = project(r#"{"rules":[{"path":"packages/**","rule":"x","language":null}]}"#);

    let error = read_project(root.path()).expect_err("refuse a null language");

    assert!(matches!(
        error,
        OcrRulesError::LanguageNotString { path, index, value }
            if path == PROJECT_RULE_FILE && index == 1 && value == "null"
    ));
}

#[test]
fn render_inlines_the_named_language_guide_without_layer_guides() {
    let root = project(
        r#"{
            "rules": [
                { "path": "crates/**", "rule": "Keep crates small." },
                { "path": "packages/**", "rule": "Use the package guide.", "language": "typescript" }
            ]
        }"#,
    );
    let rules = read_rules(root.path());
    let mut guides = guides();

    guides.layers.push(Guide {
        path: "docs/stomme/layers/react/guide.md".to_string(),
        text: "Layer rules.\n",
    });

    let rendered = render(Language::Rust, &guides, Some(&rules));
    let document = parse(&rendered);
    let untagged = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");
    let tagged = document["rules"][1]["rule"]
        .as_str()
        .expect("the rule is text");

    assert!(untagged.starts_with("Keep crates small."));
    assert!(untagged.contains("File: docs/stomme/languages/rust/guide.md"));
    assert!(untagged.contains("Layer rules."));

    assert!(tagged.starts_with("Use the package guide."));
    assert!(tagged.contains("File: docs/stomme/shared/coding.md"));
    assert!(tagged.contains("File: docs/stomme/languages/typescript/guide.md"));
    assert!(tagged.contains("TypeScript rules."));
    assert!(!tagged.contains("File: docs/stomme/languages/rust/guide.md"));
    assert!(!tagged.contains("Layer rules."));

    let language_entry = document["rules"][2]["rule"]
        .as_str()
        .expect("the rule is text");

    assert!(language_entry.contains("File: docs/stomme/languages/rust/guide.md"));
    assert!(language_entry.contains("Layer rules."));
}

#[test]
fn render_lists_a_named_language_test_patterns_once_after_the_project_language() {
    let root = project(
        r#"{
            "include": ["packages/**"],
            "rules": [
                { "path": "packages/**", "rule": "x", "language": "typescript" },
                { "path": "packages/**", "rule": "y", "language": "typescript" },
                { "path": "crates/**", "rule": "z", "language": "rust" }
            ]
        }"#,
    );
    let rules = read_rules(root.path());

    let rendered = render(Language::Rust, &guides(), Some(&rules));
    let document = parse(&rendered);

    assert_eq!(
        document["include"],
        serde_json::json!([
            "packages/**",
            "**/*_test.rs",
            "**/*_tests.rs",
            "**/tests.rs",
            "**/tests/**/*.rs",
            "**/benches/**/*.rs",
            "**/*.test.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
            "**/*.spec.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
            "**/__tests__/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
        ])
    );
}

#[test]
fn language_returns_the_guide_for_that_language() {
    let guides = guides();
    let cases = [
        (
            Language::Rust,
            "docs/stomme/languages/rust/guide.md",
            "Rust rules.\n",
        ),
        (
            Language::TypeScript,
            "docs/stomme/languages/typescript/guide.md",
            "TypeScript rules.\n",
        ),
    ];

    for (language, path, text) in cases {
        let guide = guides.language(language);

        assert_eq!(guide.path, path, "{language}");
        assert_eq!(guide.text, text, "{language}");
    }
}

fn guides() -> Guides {
    Guides {
        general: Guide {
            path: "docs/stomme/shared/coding.md".to_string(),
            text: "Shared rules.\n",
        },
        languages: LanguageGuides {
            rust: Guide {
                path: "docs/stomme/languages/rust/guide.md".to_string(),
                text: "Rust rules.\n",
            },
            typescript: Guide {
                path: "docs/stomme/languages/typescript/guide.md".to_string(),
                text: "TypeScript rules.\n",
            },
        },
        layers: Vec::new(),
    }
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
