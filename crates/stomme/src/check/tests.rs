use std::fs;

use super::*;
use crate::layer;
use crate::rule::Rule;
use crate::test_support::{complete_project, write_file};

fn assert_single_problem(problems: &[Problem], path: &str, rule: Rule, named: &str, case: &str) {
    assert_eq!(problems.len(), 1, "{case}: {problems:?}");
    assert_eq!(problems[0].path, path, "{case}");
    assert_eq!(problems[0].rule, rule, "{case}");
    assert!(
        problems[0].message.contains(named),
        "{case}: {}",
        problems[0].message
    );
}

fn check_project(root: &Path) -> Vec<Problem> {
    run(root, layer::catalog()).expect("check the project")
}

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
            clippy: layer::PolicyFile {
                text: "disallowed-methods = []\n",
                ..layer::RUST.clippy
            },
        },
        &[(
            "gpui",
            "disallowed-methods = [{ path = \"gpui::Window::refresh\", reason = \"notify instead\" }]\n",
        )],
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

#[test]
fn describe_toml_error_names_the_line_of_the_error() {
    let text = "[workspace]\nmembers = []\nresolver = \n";
    let error = toml::from_str::<toml::Table>(text).expect_err("the text is invalid TOML");

    let description = describe_toml_error(&error, text);

    assert!(description.starts_with("line 3: "), "{description}");
}

#[test]
fn check_reports_an_unreadable_root_manifest() {
    let cases = [
        ("invalid TOML", "[workspace\n", "line 1: "),
        (
            "unknown level",
            "[workspace.lints.clippy]\ntodo = \"loud\"\n",
            "line 2: ",
        ),
        (
            "level of the wrong type",
            "[workspace.lints.clippy]\ntodo = 3\n",
            "line 2: ",
        ),
        (
            "members of the wrong type",
            "[workspace]\nmembers = \"crates/*\"\n",
            "line 2: ",
        ),
        (
            "exclude of the wrong type",
            "[workspace]\nexclude = 3\n",
            "line 2: ",
        ),
    ];

    for (case, text, line) in cases {
        let root = complete_project();

        write_file(root.path(), "Cargo.toml", text);

        let problems = check_project(root.path());

        assert_single_problem(&problems, "Cargo.toml", Rule::RustConfigFile, line, case);
    }
}

#[test]
fn check_reports_a_missing_root_manifest() {
    let root = complete_project();

    fs::remove_file(root.path().join("Cargo.toml")).expect("remove Cargo.toml");

    let problems = check_project(root.path());

    assert_single_problem(&problems, "Cargo.toml", Rule::RustConfigFile, "", "missing");
}
