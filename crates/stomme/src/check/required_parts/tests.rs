use std::fs;

use super::*;
use crate::check::Rule;
use crate::test_support::{complete_project, write_file};

fn found(problems: &[Problem]) -> Vec<(&str, Rule)> {
    problems
        .iter()
        .map(|problem| (problem.path.as_str(), problem.rule))
        .collect()
}

#[test]
fn check_passes_a_complete_project() {
    let root = complete_project();

    assert_eq!(check(root.path()).expect("read the project"), []);
}

#[test]
fn check_reports_each_missing_required_part() {
    let parts = [
        "docs/README.md",
        "docs/development.md",
        "docs/adr/README.md",
        "docs/adr/TEMPLATE.md",
        "AGENTS.md",
        ".pi/tau.json",
        "renovate.json",
        "LICENSE",
    ];

    for part in parts {
        let root = complete_project();

        fs::remove_file(root.path().join(part)).expect("remove the required part");

        let problems = check(root.path()).expect("read the project");

        assert_eq!(found(&problems), [(part, Rule::RequiredPart)], "{part}");
    }
}

#[test]
fn check_reports_a_directory_in_place_of_a_required_part() {
    let root = complete_project();

    fs::remove_file(root.path().join("LICENSE")).expect("remove LICENSE");
    fs::create_dir(root.path().join("LICENSE")).expect("create a LICENSE directory");

    let problems = check(root.path()).expect("read the project");

    assert_eq!(found(&problems), [("LICENSE", Rule::RequiredPart)]);
}

#[test]
fn check_reports_a_tau_config_without_a_full_check_command() {
    let cases = [
        r#"{ "mutation": { "argv": ["cargo", "xtask", "mutation"] } }"#,
        r#"{ "fullCheck": {} }"#,
        r#"{ "fullCheck": { "argv": [] } }"#,
        r#"{ "fullCheck": { "argv": "cargo xtask check" } }"#,
        r#"{ "fullCheck": { "argv": ["cargo", 1] } }"#,
        "{ not json",
    ];

    for text in cases {
        let root = complete_project();

        write_file(root.path(), ".pi/tau.json", text);

        let problems = check(root.path()).expect("read the project");

        assert_eq!(
            found(&problems),
            [(".pi/tau.json", Rule::TauFullCheck)],
            "{text}"
        );
    }
}
