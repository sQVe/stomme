use std::fs;
use std::io;

use super::*;
use crate::layer;
use crate::rule::Rule;
use crate::test_support::{complete_project, write_file};

fn assert_single_problem(problems: &[Problem], path: &str, rule: Rule, named: &[&str], case: &str) {
    assert_eq!(problems.len(), 1, "{case}: {problems:?}");
    assert_eq!(problems[0].path, path, "{case}");
    assert_eq!(problems[0].rule, rule, "{case}");

    for fragment in named {
        assert!(
            problems[0].message.contains(fragment),
            "{case}: {}",
            problems[0].message
        );
    }
}

fn target() -> TargetBranch {
    TargetBranch::git("origin/main".to_string())
}

fn check_project(root: &Path) -> Vec<Problem> {
    run(
        root,
        layer::catalog(),
        &target(),
        &config::FileRead::filesystem(),
    )
    .expect("check the project")
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
            &format!(
                "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\nlayers = [{listed}]\n",
                env!("CARGO_PKG_VERSION")
            ),
        );

        let problems = run(
            root.path(),
            &catalog,
            &target(),
            &config::FileRead::filesystem(),
        )
        .expect("check the project");

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
            &format!(
                "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n{layers}",
                env!("CARGO_PKG_VERSION")
            ),
        );
        crate::sync_project(root.path());

        let problems = run(
            root.path(),
            &catalog,
            &target(),
            &config::FileRead::filesystem(),
        )
        .expect("check the project");
        let rules: Vec<Rule> = problems.iter().map(|problem| problem.rule).collect();

        assert_eq!(rules, expected, "{case}: {problems:?}");
    }
}

#[test]
fn run_checks_rust_files_only_for_a_valid_rust_installation() {
    let version = env!("CARGO_PKG_VERSION");
    let cases = [
        (
            "typescript",
            format!("schema = 1\nversion = \"{version}\"\nlanguage = \"typescript\"\n"),
            true,
            vec![],
        ),
        (
            "broken installation",
            "schema = 2\nlanguage = \"rust\"\n".to_string(),
            false,
            vec![Rule::BrokenInstallation],
        ),
        (
            "rust",
            format!("schema = 1\nversion = \"{version}\"\nlanguage = \"rust\"\n"),
            true,
            vec![Rule::RustConfigFile],
        ),
    ];

    for (case, config, syncs, expected) in cases {
        let root = complete_project();

        write_file(root.path(), CONFIG_FILE, &config);

        if syncs {
            crate::sync_project(root.path());
        }

        fs::remove_file(root.path().join("Cargo.toml")).expect("remove Cargo.toml");

        let problems = run(
            root.path(),
            layer::catalog(),
            &target(),
            &config::FileRead::filesystem(),
        )
        .expect("check the project");
        let rules: Vec<Rule> = problems.iter().map(|problem| problem.rule).collect();

        assert_eq!(rules, expected, "{case}: {problems:?}");
    }
}

#[test]
fn check_reports_a_role_folder_in_each_language() {
    for language in ["rust", "typescript"] {
        let root = complete_project();

        write_file(
            root.path(),
            CONFIG_FILE,
            &format!(
                "schema = 1\nversion = \"{}\"\nlanguage = \"{language}\"\n\n[roles]\ncore = \"src/domain\"\n",
                env!("CARGO_PKG_VERSION")
            ),
        );
        fs::create_dir_all(root.path().join("src/domain")).expect("create the role folder");
        crate::sync_project(root.path());

        let problems = check_project(root.path());
        let folder_problems: Vec<&Problem> = problems
            .iter()
            .filter(|problem| problem.rule == Rule::RoleFolder)
            .collect();

        assert_eq!(folder_problems.len(), 1, "{language}: {problems:?}");
        assert_eq!(folder_problems[0].path, "src/domain", "{language}");
        assert_eq!(
            folder_problems[0].key.as_deref(),
            Some("core:domain"),
            "{language}"
        );
    }
}

#[test]
fn problem_display_names_the_key() {
    let keyless = Problem {
        path: "stomme.toml".to_string(),
        rule: Rule::RoleName,
        message: "names an unknown role".to_string(),
        key: None,
    };
    let keyed = Problem {
        path: "src/domain".to_string(),
        rule: Rule::RoleFolder,
        message: "names the wrong folder".to_string(),
        key: Some("core:domain".to_string()),
    };

    assert_eq!(
        keyless.to_string(),
        "stomme.toml: names an unknown role [role-name]"
    );
    assert_eq!(
        keyed.to_string(),
        "src/domain: names the wrong folder (key `core:domain`) [role-folder]"
    );
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
        ("invalid TOML", "[workspace\n", &["line 1: "][..]),
        (
            "unknown level",
            "[workspace.lints.clippy]\ntodo = \"loud\"\n",
            &["line 2: "][..],
        ),
        (
            "level of the wrong type",
            "[workspace.lints.clippy]\ntodo = 3\n",
            &["line 2: "][..],
        ),
        (
            "members of the wrong type",
            "[workspace]\nmembers = \"crates/*\"\n",
            &["line 2: "][..],
        ),
        (
            "exclude of the wrong type",
            "[workspace]\nexclude = 3\n",
            &["line 2: "][..],
        ),
        (
            "dependency table field of the wrong type",
            "[workspace.dependencies]\nhelper = { path = \"../helper\", workspace = \"false\" }\n",
            &["line 2: "][..],
        ),
        (
            "dependency table version of the wrong type",
            "[workspace.dependencies]\nhelper = { version = 42 }\n",
            &["line 2: "][..],
        ),
        (
            "dependency shorthand of the wrong type",
            "[workspace.dependencies]\nhelper = 42\n",
            &[
                "line 2: ",
                "expected a version string or a dependency table",
            ][..],
        ),
    ];

    for (case, text, named) in cases {
        let root = complete_project();

        write_file(root.path(), "Cargo.toml", text);

        let problems = check_project(root.path());

        assert_single_problem(&problems, "Cargo.toml", Rule::RustConfigFile, named, case);
    }
}

#[test]
fn check_reports_a_toml_datetime_dependency() {
    let cases = [
        ("offset date-time", "2026-11-22T10:12:00Z"),
        ("local date-time", "2026-11-22T10:12:00"),
        ("local date", "2026-11-22"),
        ("local time", "10:12:00"),
    ];

    for (case, value) in cases {
        let root = complete_project();
        let manifest = fs::read_to_string(root.path().join("Cargo.toml")).expect("read Cargo.toml");

        write_file(
            root.path(),
            "Cargo.toml",
            &format!("{manifest}\n[workspace.dependencies]\nhelper = {value}\n"),
        );

        let problems = check_project(root.path());

        assert_single_problem(
            &problems,
            "Cargo.toml",
            Rule::RustConfigFile,
            &["a TOML date or timestamp is not a dependency"],
            case,
        );
    }
}

#[test]
fn check_accepts_string_dependency_forms() {
    let root = complete_project();
    let manifest = fs::read_to_string(root.path().join("Cargo.toml")).expect("read Cargo.toml");

    write_file(
        root.path(),
        "Cargo.toml",
        &format!(
            "{manifest}\n[workspace.dependencies]\nshorthand = \"1.0\"\ntable = {{ version = \"1.0\" }}\n"
        ),
    );

    assert_eq!(check_project(root.path()), Vec::<Problem>::new());
}

#[test]
fn check_accepts_a_dependency_table_with_fields_no_check_reads() {
    let root = complete_project();
    let manifest = fs::read_to_string(root.path().join("Cargo.toml")).expect("read Cargo.toml");

    write_file(
        root.path(),
        "Cargo.toml",
        &format!(
            "{manifest}\n[workspace.dependencies]\nempty = {{}}\nfeatured = {{ version = \"1.0\", features = [\"std\"] }}\n"
        ),
    );

    assert_eq!(check_project(root.path()), Vec::<Problem>::new());
}

#[test]
fn check_accepts_a_dependency_table_that_starts_with_the_toml_datetime_key() {
    let root = complete_project();
    let manifest = fs::read_to_string(root.path().join("Cargo.toml")).expect("read Cargo.toml");

    write_file(
        root.path(),
        "Cargo.toml",
        &format!(
            "{manifest}\n[workspace.dependencies]\nquoted = {{ \"$__toml_private_datetime\" = \"2026-11-22\", version = \"1.0\" }}\n"
        ),
    );

    assert_eq!(check_project(root.path()), Vec::<Problem>::new());
}

#[test]
fn check_fails_when_it_cannot_read_the_root_manifest() {
    let root = complete_project();

    fs::remove_file(root.path().join("Cargo.toml")).expect("remove Cargo.toml");
    fs::create_dir(root.path().join("Cargo.toml")).expect("create a directory at Cargo.toml");

    let error = run(
        root.path(),
        layer::catalog(),
        &target(),
        &config::FileRead::filesystem(),
    )
    .expect_err("a read error other than invalid UTF-8 aborts");
    let path = root.path().join("Cargo.toml");

    assert!(
        error.to_string().contains(&path.display().to_string()),
        "{error}"
    );

    let cause = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<io::Error>())
        .expect("the I/O error is in the chain");

    assert_eq!(cause.kind(), io::ErrorKind::IsADirectory);
}

#[test]
fn check_reports_a_root_manifest_that_is_not_utf8() {
    let root = complete_project();

    fs::write(root.path().join("Cargo.toml"), [0xff, 0xfe]).expect("write invalid UTF-8");

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "Cargo.toml",
        Rule::RustConfigFile,
        &["UTF-8"],
        "invalid UTF-8",
    );
}

#[test]
fn check_reports_a_missing_root_manifest() {
    let root = complete_project();

    fs::remove_file(root.path().join("Cargo.toml")).expect("remove Cargo.toml");

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "Cargo.toml",
        Rule::RustConfigFile,
        &[],
        "missing",
    );
}

#[test]
fn invalid_lock_is_reported_once() {
    let cases = [
        (
            "missing name",
            "version = 4\n\n[[package]]\nversion = \"0.0.0\"\n",
            "missing field `name`",
        ),
        (
            "missing version",
            "version = 4\n\n[[package]]\nname = \"core\"\n",
            "missing field `version`",
        ),
        (
            "ambiguous dependency",
            "version = 4\n\n[[package]]\nname = \"core\"\nversion = \"0.0.0\"\ndependencies = [\"foo\"]\n\n[[package]]\nname = \"foo\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"foo\"\nversion = \"2.0.0\"\n",
            "matches more than one lock package",
        ),
    ];

    for (case, lock, expected) in cases {
        let root = complete_project();

        write_file(
            root.path(),
            CONFIG_FILE,
            &format!(
                "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\nlayers = [\"gpui\"]\n\n[roles]\ncore = \"crates/core\"\n",
                env!("CARGO_PKG_VERSION")
            ),
        );
        write_file(
            root.path(),
            "crates/core/Cargo.toml",
            "[package]\nname = \"core\"\nversion = \"0.0.0\"\n\n[lints]\nworkspace = true\n",
        );
        write_file(root.path(), "Cargo.lock", lock);
        crate::sync_project(root.path());

        let problems = check_project(root.path());

        assert_eq!(problems.len(), 1, "{case}: {problems:?}");
        assert_eq!(problems[0].rule, Rule::RustConfigFile, "{case}");
        assert_eq!(problems[0].path, "Cargo.lock", "{case}");
        assert!(
            problems[0].message.contains(expected),
            "{case}: {problems:?}"
        );
    }
}

#[test]
fn typescript_project_does_not_read_cargo_lock() {
    let root = complete_project();

    write_file(
        root.path(),
        CONFIG_FILE,
        &format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"typescript\"\n",
            env!("CARGO_PKG_VERSION")
        ),
    );
    write_file(
        root.path(),
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"gpui\"\nversion = \"0.0.0\"\n",
    );
    crate::sync_project(root.path());

    let problems = check_project(root.path());
    let layer_problems: Vec<&Problem> = problems
        .iter()
        .filter(|problem| problem.rule == Rule::LayerDeclared)
        .collect();

    assert_eq!(layer_problems, Vec::<&Problem>::new(), "{problems:?}");
}

#[test]
fn unreadable_package_json_is_an_error() {
    let root = complete_project();

    fs::create_dir(root.path().join("package.json")).expect("make package.json a folder");

    let error = run(
        root.path(),
        layer::catalog(),
        &target(),
        &config::FileRead::filesystem(),
    )
    .expect_err("a folder named package.json cannot be read");

    assert_eq!(error.to_string(), "Could not read package.json");
}
