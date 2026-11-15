use std::fs;
use std::path::Path;

use crate::check::rust_lints;
use crate::check::{Problem, Rule};
use crate::layer;
use crate::test_support::{complete_project, write_file};

const MEMBER_WITHOUT_LINTS: &str = "[package]\nname = \"two\"\nversion = \"0.0.0\"\n";

fn check_project(root: &Path) -> Vec<Problem> {
    rust_lints::check(root, layer::catalog().rust_lints()).expect("read the project")
}

fn replace_in_manifest(root: &Path, from: &str, to: &str) {
    let path = root.join("Cargo.toml");
    let manifest = fs::read_to_string(&path).expect("read Cargo.toml");

    assert!(manifest.contains(from), "Cargo.toml has {from:?}");
    fs::write(&path, manifest.replacen(from, to, 1)).expect("write Cargo.toml");
}

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

#[test]
fn check_reports_a_member_that_does_not_inherit_the_lints() {
    let root = complete_project();

    write_file(root.path(), "crates/two/Cargo.toml", MEMBER_WITHOUT_LINTS);

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "crates/two/Cargo.toml",
        Rule::RustLintInheritance,
        "workspace = true",
        "member under crates/*",
    );
}

#[test]
fn check_skips_an_excluded_member() {
    let root = complete_project();

    write_file(root.path(), "crates/two/Cargo.toml", MEMBER_WITHOUT_LINTS);
    replace_in_manifest(
        root.path(),
        "members = [\"crates/*\"]",
        "members = [\"crates/*\"]\nexclude = [\"crates/two\"]",
    );

    assert_eq!(check_project(root.path()), []);
}

#[test]
fn check_reports_a_root_package_that_does_not_inherit_the_lints() {
    let root = complete_project();

    replace_in_manifest(
        root.path(),
        "[workspace]",
        "[package]\nname = \"root\"\nversion = \"0.0.0\"\n\n[workspace]",
    );

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "Cargo.toml",
        Rule::RustLintInheritance,
        "workspace = true",
        "root package",
    );
}

#[test]
fn check_expands_literal_members_and_star_inside_a_segment() {
    let root = complete_project();

    write_file(root.path(), "tools/xtask/Cargo.toml", MEMBER_WITHOUT_LINTS);
    write_file(root.path(), "apps/app-one/Cargo.toml", MEMBER_WITHOUT_LINTS);
    write_file(root.path(), "apps/other/Cargo.toml", MEMBER_WITHOUT_LINTS);
    replace_in_manifest(
        root.path(),
        "members = [\"crates/*\"]",
        "members = [\"./crates/*\", \"tools/xtask\", \"apps/app-*\"]",
    );

    let problems = check_project(root.path());
    let paths: Vec<&str> = problems
        .iter()
        .map(|problem| problem.path.as_str())
        .collect();

    assert_eq!(
        paths,
        ["apps/app-one/Cargo.toml", "tools/xtask/Cargo.toml"],
        "{problems:?}"
    );
}

#[test]
fn check_reports_a_member_pattern_it_cannot_expand() {
    for pattern in ["crates/[ab]", "crates/?", "crates/**", "crates/{a,b}"] {
        let root = complete_project();

        replace_in_manifest(
            root.path(),
            "members = [\"crates/*\"]",
            &format!("members = [\"crates/*\", \"{pattern}\"]"),
        );

        let problems = check_project(root.path());

        assert_single_problem(
            &problems,
            "Cargo.toml",
            Rule::RustConfigFile,
            &format!("`{pattern}`"),
            pattern,
        );
    }
}

#[test]
fn check_reports_a_member_manifest_it_cannot_read() {
    let cases = [
        ("missing manifest", "tools/xtask", None),
        ("invalid TOML", "tools/xtask", Some("[package\n")),
        (
            "lints.workspace of the wrong type",
            "tools/xtask",
            Some("[lints]\nworkspace = \"yes\"\n"),
        ),
    ];

    for (case, member, text) in cases {
        let root = complete_project();

        fs::create_dir_all(root.path().join(member)).expect("create the member folder");
        if let Some(text) = text {
            write_file(root.path(), &format!("{member}/Cargo.toml"), text);
        }
        replace_in_manifest(
            root.path(),
            "members = [\"crates/*\"]",
            &format!("members = [\"crates/*\", \"{member}\"]"),
        );

        let problems = check_project(root.path());

        assert_single_problem(
            &problems,
            &format!("{member}/Cargo.toml"),
            Rule::RustConfigFile,
            "",
            case,
        );
    }
}

#[test]
fn check_reads_a_member_pattern_that_starts_with_a_dot_segment() {
    let root = complete_project();

    write_file(root.path(), "crates/two/Cargo.toml", MEMBER_WITHOUT_LINTS);
    replace_in_manifest(
        root.path(),
        "members = [\"crates/*\"]",
        "members = [\"./crates/*\"]",
    );

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "crates/two/Cargo.toml",
        Rule::RustLintInheritance,
        "workspace = true",
        "./crates/*",
    );
}

#[test]
fn check_matches_each_part_between_two_stars_in_order() {
    let root = complete_project();

    write_file(root.path(), "apps/one-cli/Cargo.toml", MEMBER_WITHOUT_LINTS);
    write_file(
        root.path(),
        "apps/one-two-cli/Cargo.toml",
        MEMBER_WITHOUT_LINTS,
    );
    replace_in_manifest(
        root.path(),
        "members = [\"crates/*\"]",
        "members = [\"apps/*-*-cli\"]",
    );

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "apps/one-two-cli/Cargo.toml",
        Rule::RustLintInheritance,
        "workspace = true",
        "apps/*-*-cli",
    );
}

const MEMBER_WITH_LINTS: &str =
    "[package]\nname = \"cli\"\nversion = \"0.0.0\"\n\n[lints]\nworkspace = true\n";

fn write_cli_dependencies(root: &Path, dependencies: &str) {
    write_file(
        root,
        "crates/cli/Cargo.toml",
        &format!("{MEMBER_WITH_LINTS}\n{dependencies}"),
    );
}

#[test]
fn check_reports_an_unlisted_path_dependency_that_does_not_inherit_the_lints() {
    let root = complete_project();

    write_cli_dependencies(
        root.path(),
        "[dependencies]\nhelper = { path = \"../../helper\" }\n",
    );
    write_file(root.path(), "helper/Cargo.toml", MEMBER_WITHOUT_LINTS);

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "helper/Cargo.toml",
        Rule::RustLintInheritance,
        "workspace = true",
        "path dependency",
    );
}

#[test]
fn check_follows_path_dependencies_in_every_dependency_table() {
    let cases = [
        ("dev-dependencies", "[dev-dependencies]\n"),
        ("build-dependencies", "[build-dependencies]\n"),
        ("target dependencies", "[target.'cfg(unix)'.dependencies]\n"),
        (
            "target dev-dependencies",
            "[target.'cfg(unix)'.dev-dependencies]\n",
        ),
    ];

    for (case, table) in cases {
        let root = complete_project();

        write_cli_dependencies(
            root.path(),
            &format!("{table}helper = {{ path = \"../../helper\" }}\n"),
        );
        write_file(root.path(), "helper/Cargo.toml", MEMBER_WITHOUT_LINTS);

        let problems = check_project(root.path());

        assert_single_problem(
            &problems,
            "helper/Cargo.toml",
            Rule::RustLintInheritance,
            "workspace = true",
            case,
        );
    }
}

#[test]
fn check_follows_an_inherited_workspace_path_dependency() {
    let root = complete_project();

    replace_in_manifest(
        root.path(),
        "members = [\"crates/*\"]",
        "members = [\"crates/*\"]\n\n[workspace.dependencies]\nhelper = { path = \"helper\" }\nunused = { path = \"unused\" }",
    );
    write_cli_dependencies(root.path(), "[dependencies]\nhelper.workspace = true\n");
    write_file(root.path(), "helper/Cargo.toml", MEMBER_WITHOUT_LINTS);
    write_file(root.path(), "unused/Cargo.toml", MEMBER_WITHOUT_LINTS);

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "helper/Cargo.toml",
        Rule::RustLintInheritance,
        "workspace = true",
        "inherited path dependency",
    );
}

#[test]
fn check_follows_path_dependencies_transitively() {
    let root = complete_project();

    write_cli_dependencies(
        root.path(),
        "[dependencies]\nhelper = { path = \"../../helper\" }\n",
    );
    write_file(
        root.path(),
        "helper/Cargo.toml",
        &format!("{MEMBER_WITH_LINTS}\n[dependencies]\ninner = {{ path = \"../inner\" }}\n"),
    );
    write_file(root.path(), "inner/Cargo.toml", MEMBER_WITHOUT_LINTS);

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "inner/Cargo.toml",
        Rule::RustLintInheritance,
        "workspace = true",
        "transitive path dependency",
    );
}

#[test]
fn check_follows_an_absolute_path_dependency_inside_the_root() {
    let root = complete_project();
    let helper = root.path().join("helper");

    write_cli_dependencies(
        root.path(),
        &format!(
            "[dependencies]\nhelper = {{ path = \"{}\" }}\n",
            helper.display()
        ),
    );
    write_file(root.path(), "helper/Cargo.toml", MEMBER_WITHOUT_LINTS);

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "helper/Cargo.toml",
        Rule::RustLintInheritance,
        "workspace = true",
        "absolute path dependency",
    );
}

#[test]
fn check_skips_a_path_dependency_outside_the_root_or_excluded() {
    let outside = tempfile::tempdir().expect("create an outside folder");

    write_file(outside.path(), "Cargo.toml", MEMBER_WITHOUT_LINTS);

    let cases = [
        (
            "relative path outside the root",
            "../../../outside".to_string(),
        ),
        (
            "absolute path outside the root",
            outside.path().display().to_string(),
        ),
        ("excluded path", "../../skipped".to_string()),
    ];

    for (case, path) in cases {
        let root = complete_project();

        replace_in_manifest(
            root.path(),
            "members = [\"crates/*\"]",
            "members = [\"crates/*\"]\nexclude = [\"skipped\"]",
        );
        write_cli_dependencies(
            root.path(),
            &format!("[dependencies]\nhelper = {{ path = \"{path}\" }}\n"),
        );
        write_file(
            root.path(),
            "skipped/Cargo.toml",
            "[package]\nname = \"skipped\"\nversion = \"0.0.0\"\n\n[dependencies]\ninner = { path = \"../inner\" }\n",
        );
        write_file(root.path(), "inner/Cargo.toml", MEMBER_WITHOUT_LINTS);

        assert_eq!(check_project(root.path()), [], "{case}");
    }
}
