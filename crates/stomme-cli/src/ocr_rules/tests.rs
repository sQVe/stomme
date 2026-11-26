use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::*;
use crate::rule::{Enforcement, Rule};

#[test]
fn render_without_project_rules_writes_one_language_entry() {
    let rendered = render(Language::Rust, &guides(), &BTreeMap::new(), None);
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

    let rendered = render(
        Language::TypeScript,
        &guides(),
        &BTreeMap::new(),
        Some(&rules),
    );
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
    let rendered = render(Language::Rust, &guides(), &BTreeMap::new(), None);

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

    let rendered = render(Language::Rust, &guides(), &BTreeMap::new(), Some(&rules));
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
    let rendered = render(Language::TypeScript, &guides(), &BTreeMap::new(), None);
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

    let rendered = render(Language::Rust, &guides(), &BTreeMap::new(), Some(&rules));

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

    let rendered = render(Language::Rust, &guides, &BTreeMap::new(), Some(&rules));
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

    let rendered = render(Language::Rust, &guides(), &BTreeMap::new(), Some(&rules));
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

#[test]
fn render_scopes_role_clauses_to_declared_roles_in_order() {
    let roles = roles(&[
        ("ui", "src/ui"),
        ("app", "crates/app"),
        ("core", "crates/core"),
    ]);

    let rendered = render(Language::Rust, &embedded_guides(), &roles, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");

    assert_eq!(
        clause_roots(text),
        ["crates/core/", "src/ui/", "crates/app/"]
    );
    assert!(!text.contains("starts with crates/kit/"));
    assert!(!text.contains("starts with crates/cli/"));
}

#[test]
fn role_clause_root_normalizes_and_keeps_one_trailing_slash() {
    let roles = roles(&[("ui", "src/ui"), ("app", "./crates/app/")]);

    let rendered = render(Language::Rust, &embedded_guides(), &roles, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");

    assert_eq!(clause_roots(text), ["src/ui/", "crates/app/"]);
}

#[test]
fn a_role_with_no_normalized_root_gets_no_clause() {
    let declared = roles(&[("cli", "."), ("ui", "src/ui")]);
    let rendered = render(Language::Rust, &embedded_guides(), &declared, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");

    assert_eq!(clause_roots(text), ["src/ui/"]);
    assert!(!text.contains("If the file path starts with /,"));
}

#[test]
fn role_clause_text_comes_from_the_marked_adr_and_selected_layers() {
    let roles = roles(&[("app", "crates/app"), ("kit", "crates/kit")]);
    let mut guides = embedded_guides();

    guides
        .layers
        .retain(|guide| guide.path.ends_with("/gpui/guide.md"));

    let rendered = render(Language::Rust, &guides, &roles, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");
    let app = clause(text, "crates/app/");
    let kit = clause(text, "crates/kit/");

    assert!(app.contains("`app` is the graphical entry point"));
    assert!(app.contains("`gpui-app-lifetime`"));
    assert!(app.contains("owns the GPUI application"));

    assert!(kit.contains("`kit` holds reusable UI building blocks"));
    assert!(!kit.contains("`gpui-component-kit`"));

    let full = embedded_guides();
    let rendered = render(Language::Rust, &full, &roles, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");
    let kit = clause(text, "crates/kit/");

    assert!(kit.contains("`kit` holds reusable UI building blocks"));
    assert!(!kit.contains("`gpui-component-kit`"));
    assert_eq!(text.matches("`gpui-component-kit`").count(), 1);
    assert!(text.contains("`gpui-component-kit`"));
}

#[test]
fn rendered_rule_text_keeps_partially_checked_rules() {
    let declared = roles(&[("core", "crates/core"), ("kit", "crates/kit")]);
    let rendered = render(Language::Rust, &embedded_guides(), &declared, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");

    for kept in [
        "react-component-file",
        "react-test-placement",
        "gpui-plain-core",
        "gpui-component-kit",
    ] {
        assert!(text.contains(kept), "{kept}");
    }

    let rendered = render(Language::TypeScript, &embedded_guides(), &declared, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");

    assert!(text.contains("A unit test sits beside its owner as"));
}

#[test]
fn marked_role_blocks_leave_the_global_guide_text() {
    let declared = roles(&[("app", "crates/app"), ("kit", "crates/kit")]);
    let rendered = render(Language::Rust, &embedded_guides(), &declared, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");

    assert_eq!(text.matches("`gpui-app-lifetime`").count(), 1);
    assert!(clause(text, "crates/app/").contains("`gpui-app-lifetime`"));
    assert_eq!(text.matches("`gpui-component-kit`").count(), 1);
    assert!(text.contains("`gpui-component-kit`"));
    assert!(!clause(text, "crates/kit/").contains("`gpui-component-kit`"));

    let without_app = roles(&[("kit", "crates/kit")]);
    let rendered = render(Language::Rust, &embedded_guides(), &without_app, None);
    let document = parse(&rendered);
    let text = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");

    assert!(!text.contains("`gpui-app-lifetime`"));
}

#[test]
fn an_unterminated_marker_adds_no_role_text() {
    let text = "- `gpui-app-lifetime`: Own the app lifetime. <!-- role:app\n";

    assert!(marked_blocks(text, "app").is_empty());
    assert_eq!(without_marked_blocks(text), text.trim_end());
}

#[test]
fn a_tilde_fenced_marker_adds_no_role_text() {
    let text = "- `gpui-app-lifetime`: Own the app lifetime. <!-- role:app -->\n\n~~~markdown\n<!-- role:app -->\n~~~\n\nOutside the fence.\n";
    let expected = "\n~~~markdown\n<!-- role:app -->\n~~~\n\nOutside the fence.";

    assert_eq!(
        marked_blocks(text, "app"),
        ["`gpui-app-lifetime`: Own the app lifetime."]
    );
    assert_eq!(without_marked_blocks(text), expected);
}

#[test]
fn a_backtick_fenced_marker_adds_no_role_text() {
    let text = "- `gpui-app-lifetime`: Own the app lifetime. <!-- role:app -->\n\n```markdown\n<!-- role:app -->\n```\n\nOutside the fence.\n";
    let expected = "\n```markdown\n<!-- role:app -->\n```\n\nOutside the fence.";

    assert_eq!(
        marked_blocks(text, "app"),
        ["`gpui-app-lifetime`: Own the app lifetime."]
    );
    assert_eq!(without_marked_blocks(text), expected);
}

#[test]
fn a_marker_after_a_closed_fence_adds_role_text() {
    let text = "```markdown\n<!-- role:app -->\n```\n\n- `gpui-app-lifetime`: Own the app lifetime. <!-- role:app -->\n";

    assert_eq!(
        marked_blocks(text, "app"),
        ["`gpui-app-lifetime`: Own the app lifetime."]
    );
}

#[test]
fn a_marker_followed_by_prose_adds_no_role_text() {
    let text = "- Use <!-- role:app --> in prose.\n";

    assert!(marked_blocks(text, "app").is_empty());
    assert_eq!(
        without_marked_blocks(text),
        "- Use <!-- role:app --> in prose."
    );
}

#[test]
fn a_marker_on_another_list_form_adds_role_text() {
    let cases = [
        ("star", "* `gpui-app-lifetime`: Own the app lifetime."),
        ("plus", "+ `gpui-app-lifetime`: Own the app lifetime."),
        ("ordered", "1. `gpui-app-lifetime`: Own the app lifetime."),
        ("indented", "  - `gpui-app-lifetime`: Own the app lifetime."),
    ];

    for (case, item) in cases {
        let text = format!("{item} <!-- role:app -->\n");

        assert_eq!(
            marked_blocks(&text, "app"),
            ["`gpui-app-lifetime`: Own the app lifetime."],
            "{case}"
        );
        assert_eq!(without_marked_blocks(&text), "", "{case}");
    }
}

#[test]
#[should_panic(expected = "a marked block is a top-level list item, not a nested one")]
fn a_marker_on_a_nested_list_item_is_refused() {
    marked_blocks("- parent\n  - child <!-- role:core -->\n", "core");
}

#[test]
#[should_panic(expected = "a marked block is a top-level list item, not a nested one")]
fn a_marker_on_a_four_space_nested_list_item_is_refused() {
    marked_blocks("- parent\n    - child <!-- role:core -->\n", "core");
}

#[test]
#[should_panic(expected = "a marked block is a top-level list item, not a nested one")]
fn a_marker_on_a_nested_item_continuation_is_refused() {
    marked_blocks("- parent\n  - child\n    more <!-- role:core -->\n", "core");
}

#[test]
#[should_panic(expected = "a marked block is a list item whose marker ends its last line")]
fn a_marker_before_a_fenced_continuation_is_refused() {
    marked_blocks(
        "- `core`: Text. <!-- role:core -->\n  ```markdown\n  code\n  ```\n",
        "core",
    );
}

#[test]
#[should_panic(expected = "a marked block is a list item whose marker ends its last line")]
fn a_marker_before_a_tab_indented_continuation_is_refused() {
    marked_blocks("- `core`: Text. <!-- role:core -->\n\tMore.\n", "core");
}

#[test]
fn a_marker_before_a_thematic_break_or_heading_adds_role_text() {
    for next in ["---", "* * *", "# Heading", "> quote"] {
        let text = format!("- `gpui-app-lifetime`: Own it. <!-- role:app -->\n{next}\n");

        assert_eq!(
            marked_blocks(&text, "app"),
            ["`gpui-app-lifetime`: Own it."],
            "{next}"
        );
    }
}

#[test]
fn a_multi_paragraph_block_keeps_its_relative_indentation() {
    let text = "  - `gpui-app-lifetime`: First.\n\n    Second.\n\n      Third. <!-- role:app -->\n";

    assert_eq!(
        marked_blocks(text, "app"),
        ["`gpui-app-lifetime`: First.\n\nSecond.\n\n  Third."]
    );
}

#[test]
fn a_tab_indented_body_line_is_written_as_spaces() {
    let text = "- `gpui-app-lifetime`: First.\n\tSecond. <!-- role:app -->\n";

    assert_eq!(
        marked_blocks(text, "app"),
        ["`gpui-app-lifetime`: First.\n  Second."]
    );
}

#[test]
#[should_panic(expected = "a marked block is a list item whose marker ends its last line")]
fn a_marker_before_a_lazy_paragraph_line_is_refused() {
    marked_blocks("- `core`: Text. <!-- role:core -->\n More.\n", "core");
}

#[test]
fn a_marker_before_a_line_short_of_the_content_column_adds_role_text() {
    let text = "- `gpui-app-lifetime`: Own the app lifetime. <!-- role:app -->\n ```\n";

    assert_eq!(
        marked_blocks(text, "app"),
        ["`gpui-app-lifetime`: Own the app lifetime."]
    );
}

#[test]
#[should_panic(expected = "unknown role")]
fn without_marked_blocks_refuses_an_unknown_role_marker() {
    without_marked_blocks("- `x`: Text. <!-- role:clii -->\n");
}

#[test]
#[should_panic(expected = "a marked block is a list item whose marker ends its last line")]
fn a_marker_on_its_own_line_is_refused() {
    marked_blocks("Text.\n\n<!-- role:core -->\n", "core");
}

#[test]
#[should_panic(expected = "a marked block is a list item whose marker ends its last line")]
fn a_marker_before_a_continuation_line_is_refused() {
    marked_blocks("- `core`: Text. <!-- role:core -->\n  More.\n", "core");
}

#[test]
#[should_panic(expected = "a marked block is a list item whose marker ends its last line")]
fn a_marker_before_a_blank_line_and_a_continuation_is_refused() {
    marked_blocks("- `core`: Text. <!-- role:core -->\n\n  More.\n", "core");
}

#[test]
fn a_marker_before_a_blank_line_and_a_new_item_adds_role_text() {
    let text = "- `gpui-app-lifetime`: Own the app lifetime. <!-- role:app -->\n\n- Next.\n";

    assert_eq!(
        marked_blocks(text, "app"),
        ["`gpui-app-lifetime`: Own the app lifetime."]
    );
}

#[test]
fn a_marker_on_an_indented_item_after_a_heading_adds_role_text() {
    let text = "# Group\n\n  - `gpui-app-lifetime`: Own the app lifetime. <!-- role:app -->\n";

    assert_eq!(
        marked_blocks(text, "app"),
        ["`gpui-app-lifetime`: Own the app lifetime."]
    );
}

#[test]
#[should_panic(expected = "unknown role")]
fn marked_blocks_refuses_an_unknown_role_marker() {
    marked_blocks("- `x`: Text. <!-- role:server -->\n", "core");
}

#[test]
fn role_text_holds_no_exactly_enforced_rule() {
    let files = crate::owned::bundle_set(crate::layer::catalog());
    let mut offenders = Vec::new();

    let gpui = include_str!("../../../../foundation/layers/gpui/guide.md");
    let app_text = marked_blocks(gpui, "app").join("\n");

    assert!(
        rule_ids(&app_text).contains(&"gpui-app-lifetime"),
        "{app_text}"
    );

    for file in &files {
        for role in ROLES {
            for block in marked_blocks(&file.content, role) {
                for id in rule_ids(&block) {
                    let Some(rule) = Rule::ALL.iter().find(|rule| rule.id() == id) else {
                        continue;
                    };

                    if matches!(
                        rule.metadata().enforcement,
                        Enforcement::Enforced(_) | Enforcement::RustAndTypeScript { .. }
                    ) {
                        offenders.push(format!("{}: {id}", file.path));
                    }
                }
            }
        }
    }

    assert!(offenders.is_empty(), "{offenders:?}");
}

#[test]
fn every_closed_role_has_a_marked_adr_block() {
    for role in ROLES {
        assert!(
            !marked_blocks(ROLE_ADR, role).is_empty(),
            "the role `{role}` has no marked block in ADR 0004"
        );
    }
}

#[test]
fn project_entry_puts_its_own_text_before_the_clauses() {
    let root = project(r#"{"rules":[{"path":"crates/**","rule":"Keep crates small."}]}"#);
    let rules = read_rules(root.path());
    let roles = roles(&[("cli", "crates/stomme-cli")]);

    let rendered = render(Language::Rust, &embedded_guides(), &roles, Some(&rules));
    let document = parse(&rendered);
    let entry = document["rules"][0]["rule"]
        .as_str()
        .expect("the rule is text");
    let own = entry
        .find("Keep crates small.")
        .expect("the own text comes first");
    let guides = entry
        .find("File: docs/stomme/shared/coding.md")
        .expect("the shared guide is inlined");
    let clause = entry
        .find("If the file path starts with crates/stomme-cli/, apply:")
        .expect("the clause is inlined");
    let outro = entry
        .find("Also apply the rules in the project's AGENTS.md")
        .expect("the outro is inlined");

    assert!(own < guides);
    assert!(guides < clause);
    assert!(clause < outro);
}

fn roles(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(role, path)| (role.to_string(), path.to_string()))
        .collect()
}

fn clause_roots(text: &str) -> Vec<String> {
    let prefix = "If the file path starts with ";

    text.match_indices(prefix)
        .map(|(index, _)| {
            let rest = &text[index + prefix.len()..];
            let end = rest.find(", apply:").expect("a clause names its root");

            rest[..end].to_string()
        })
        .collect()
}

fn clause<'a>(text: &'a str, root: &str) -> &'a str {
    let start = text
        .find(&format!("If the file path starts with {root}, apply: "))
        .expect("the role has a clause");
    let rest = &text[start..];
    let next_clause = rest.find("\n\nIf the file path starts with ");
    let outro = rest.find("\n\nAlso apply the rules");
    let end = match (next_clause, outro) {
        (Some(next), Some(outro)) => Some(next.min(outro)),
        (next, outro) => next.or(outro),
    }
    .map_or(text.len(), |offset| start + offset);

    &text[start..end]
}

fn rule_ids(text: &str) -> Vec<&str> {
    text.lines()
        .map(|line| crate::fence::list_item_content(line).unwrap_or(line))
        .filter_map(|line| line.strip_prefix('`'))
        .filter_map(|rest| rest.split_once("`: "))
        .map(|(id, _)| id)
        .collect()
}

fn embedded_guides() -> Guides {
    Guides {
        general: Guide {
            path: "docs/stomme/shared/coding.md".to_string(),
            text: include_str!("../../../../foundation/shared/coding.md"),
        },
        languages: LanguageGuides {
            rust: Guide {
                path: "docs/stomme/languages/rust/guide.md".to_string(),
                text: include_str!("../../../../foundation/languages/rust/guide.md"),
            },
            typescript: Guide {
                path: "docs/stomme/languages/typescript/guide.md".to_string(),
                text: include_str!("../../../../foundation/languages/typescript/guide.md"),
            },
        },
        layers: vec![
            Guide {
                path: "docs/stomme/layers/gpui/guide.md".to_string(),
                text: include_str!("../../../../foundation/layers/gpui/guide.md"),
            },
            Guide {
                path: "docs/stomme/layers/gpui-component/guide.md".to_string(),
                text: include_str!("../../../../foundation/layers/gpui-component/guide.md"),
            },
            Guide {
                path: "docs/stomme/layers/react/guide.md".to_string(),
                text: include_str!("../../../../foundation/layers/react/guide.md"),
            },
        ],
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
