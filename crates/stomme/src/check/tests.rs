use std::fs;

use super::*;
use crate::layer;
use crate::test_support::{complete_project, write_file};

#[test]
fn run_reports_a_conflicting_pair_in_either_order() {
    let catalog = layer::test_catalog(&[
        ("one", "language = \"rust\"\nconflicts = [\"two\"]\n"),
        ("two", "language = \"rust\"\n"),
    ]);

    for listed in ["\"one\", \"two\"", "\"two\", \"one\""] {
        let root = complete_project();

        write_file(
            root.path(),
            CONFIG_FILE,
            &format!("schema = 1\nlanguage = \"rust\"\nlayers = [{listed}]\n"),
        );

        let problems = run(root.path(), &catalog).expect("check the project");

        assert_eq!(problems.len(), 1, "{listed}: {problems:?}");
        assert_eq!(problems[0].path, CONFIG_FILE, "{listed}");
        assert_eq!(problems[0].rule, Rule::BrokenInstallation, "{listed}");
        for name in ["`one`", "`two`"] {
            assert!(
                problems[0].message.contains(name),
                "{listed}: {name}: {}",
                problems[0].message
            );
        }
    }
}

#[test]
fn run_requires_gpui_bans_only_when_stomme_toml_selects_gpui() {
    let catalog = layer::test_policy_catalog(
        &layer::RustSource {
            lints: layer::RUST.lints,
            clippy: "disallowed-methods = []\n",
        },
        "disallowed-methods = [{ path = \"gpui::Window::refresh\", reason = \"notify instead\" }]\n",
    );
    let cases = [
        (
            "gpui selected",
            "layers = [\"gpui\"]\n",
            vec![Rule::GpuiRequiredBan],
        ),
        ("no layers", "", vec![]),
    ];

    for (case, layers, expected) in cases {
        let root = complete_project();

        write_file(
            root.path(),
            CONFIG_FILE,
            &format!("schema = 1\nlanguage = \"rust\"\n{layers}"),
        );

        let problems = run(root.path(), &catalog).expect("check the project");
        let rules: Vec<Rule> = problems.iter().map(|problem| problem.rule).collect();

        assert_eq!(rules, expected, "{case}: {problems:?}");
    }
}

#[test]
fn run_checks_rust_files_only_for_a_valid_rust_installation() {
    let cases = [
        (
            "typescript",
            "schema = 1\nlanguage = \"typescript\"\n",
            vec![],
        ),
        (
            "broken installation",
            "schema = 2\nlanguage = \"rust\"\n",
            vec![Rule::BrokenInstallation],
        ),
        (
            "rust",
            "schema = 1\nlanguage = \"rust\"\n",
            vec![Rule::RustConfigFile],
        ),
    ];

    for (case, config, expected) in cases {
        let root = complete_project();

        write_file(root.path(), CONFIG_FILE, config);
        fs::remove_file(root.path().join("Cargo.toml")).expect("remove Cargo.toml");

        let problems = run(root.path(), layer::catalog()).expect("check the project");
        let rules: Vec<Rule> = problems.iter().map(|problem| problem.rule).collect();

        assert_eq!(rules, expected, "{case}: {problems:?}");
    }
}
