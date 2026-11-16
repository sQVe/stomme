use std::collections::BTreeSet;

use super::*;
use crate::layer::{self, Catalog, PolicyFile, RUST, RustSource};
use crate::test_support::{complete_project, write_file};

const RUST_BANS: &str = r#"disallowed-methods = [
  { path = "std::env::set_var", reason = "the environment is shared between threads" },
]
"#;

const GPUI_BANS: &str = r#"disallowed-methods = [
  { path = "gpui::Window::refresh", reason = "notify the entity instead" },
]
"#;

const REQUIRED_RUST_BAN: &str =
    r#"{ path = "std::env::set_var", reason = "the environment is shared between threads" }"#;

fn policy_catalog() -> Catalog {
    layer::test_policy_catalog(
        &RustSource {
            lints: RUST.lints,
            clippy: PolicyFile {
                text: RUST_BANS,
                ..RUST.clippy
            },
        },
        GPUI_BANS,
    )
}

fn check_project(root: &Path, catalog: &Catalog, layers: &[&str]) -> Vec<Problem> {
    let layers: BTreeSet<String> = layers.iter().map(ToString::to_string).collect();

    check(root, &catalog.required_bans(&layers)).expect("read the project")
}

fn write_bans(root: &Path, entries: &[&str]) {
    let lines: String = entries
        .iter()
        .map(|entry| format!("  {entry},\n"))
        .collect();
    let text = format!("disallowed-methods = [\n{lines}]\n");

    write_file(root, CLIPPY_FILE, &text);
}

fn found(problems: &[Problem]) -> Vec<(&str, Rule)> {
    problems
        .iter()
        .map(|problem| (problem.path.as_str(), problem.rule))
        .collect()
}

#[test]
fn check_passes_without_required_bans_or_clippy_toml() {
    let root = complete_project();

    assert_eq!(check_project(root.path(), layer::catalog(), &["gpui"]), []);
}

#[test]
fn check_reports_a_missing_required_ban_naming_it() {
    let cases: [(&str, Option<&[&str]>); 4] = [
        ("no clippy.toml", None),
        ("no entries", Some(&[])),
        (
            "other ban",
            Some(&[r#"{ path = "std::process::exit", reason = "return an error" }"#]),
        ),
        ("path without a reason", Some(&[r#""std::env::set_var""#])),
    ];

    for (case, entries) in cases {
        let root = complete_project();

        if let Some(entries) = entries {
            write_bans(root.path(), entries);
        }

        let problems = check_project(root.path(), &policy_catalog(), &[]);

        assert_eq!(
            found(&problems),
            [(CLIPPY_FILE, Rule::RustRequiredBan)],
            "{case}"
        );
        assert!(
            problems[0].message.contains("`std::env::set_var`"),
            "{case}: {}",
            problems[0].message
        );
    }
}

#[test]
fn check_reports_a_required_ban_with_another_reason() {
    let root = complete_project();

    write_bans(
        root.path(),
        &[r#"{ path = "std::env::set_var", reason = "we like it" }"#],
    );

    let problems = check_project(root.path(), &policy_catalog(), &[]);

    assert_eq!(found(&problems), [(CLIPPY_FILE, Rule::RustRequiredBan)]);
    assert!(
        problems[0].message.contains("`std::env::set_var`"),
        "{}",
        problems[0].message
    );
}

#[test]
fn check_passes_required_bans_next_to_project_bans() {
    let root = complete_project();

    write_bans(
        root.path(),
        &[
            r#""std::process::exit""#,
            REQUIRED_RUST_BAN,
            r#"{ path = "std::fs::remove_dir_all", reason = "use trash", replacement = "trash::delete" }"#,
        ],
    );

    assert_eq!(check_project(root.path(), &policy_catalog(), &[]), []);
}

#[test]
fn check_requires_gpui_bans_only_with_the_gpui_layer() {
    let catalog = policy_catalog();
    let root = complete_project();

    write_bans(root.path(), &[REQUIRED_RUST_BAN]);

    let with_gpui = check_project(root.path(), &catalog, &["gpui"]);
    let without_gpui = check_project(root.path(), &catalog, &[]);

    assert_eq!(found(&with_gpui), [(CLIPPY_FILE, Rule::GpuiRequiredBan)]);
    assert!(
        with_gpui[0].message.contains("`gpui::Window::refresh`"),
        "{}",
        with_gpui[0].message
    );
    assert_eq!(without_gpui, []);
}

#[test]
fn check_reports_a_clippy_toml_it_cannot_read() {
    let cases = [
        ("invalid TOML", "disallowed-methods = [\n"),
        ("entry of the wrong type", "disallowed-methods = [3]\n"),
    ];

    for (case, text) in cases {
        let root = complete_project();

        write_file(root.path(), CLIPPY_FILE, text);

        let problems = check_project(root.path(), &policy_catalog(), &[]);

        assert_eq!(
            found(&problems),
            [(CLIPPY_FILE, Rule::RustConfigFile)],
            "{case}"
        );
    }
}
