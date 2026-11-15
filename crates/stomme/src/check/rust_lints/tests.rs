use super::*;
use crate::layer;
use crate::test_support::{complete_project, write_file};

const MEMBER_WITHOUT_LINTS: &str = "[package]\nname = \"two\"\nversion = \"0.0.0\"\n";

fn check_project(root: &Path) -> Vec<Problem> {
    check(root, layer::catalog().rust_lints()).expect("read the project")
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
fn check_passes_a_complete_project() {
    let root = complete_project();

    assert_eq!(check_project(root.path()), []);
}

#[test]
fn check_reports_a_shared_lint_set_to_a_lower_level() {
    let root = complete_project();

    replace_in_manifest(root.path(), "todo = \"warn\"", "todo = \"allow\"");

    let problems = check_project(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, "Cargo.toml");
    assert_eq!(problems[0].rule, Rule::RustLintLevel);
    assert!(
        problems[0].message.contains("`clippy::todo`"),
        "{}",
        problems[0].message
    );
}

#[test]
fn check_reports_a_weakened_shared_lint_naming_it() {
    let cases = [
        (
            "missing clippy lint",
            "dbg_macro = \"warn\"\n",
            "",
            "`clippy::dbg_macro`",
        ),
        (
            "missing rust lint",
            "unsafe_code = \"deny\"\n",
            "",
            "`unsafe_code`",
        ),
        (
            "lower rust level",
            "unsafe_code = \"deny\"",
            "unsafe_code = { level = \"warn\" }",
            "`unsafe_code`",
        ),
        (
            "other priority",
            "all = { level = \"warn\", priority = -2 }",
            "all = { level = \"warn\", priority = -3 }",
            "`clippy::all`",
        ),
        (
            "missing priority",
            "all = { level = \"warn\", priority = -2 }",
            "all = \"warn\"",
            "`clippy::all`",
        ),
    ];

    for (case, from, to, named) in cases {
        let root = complete_project();

        replace_in_manifest(root.path(), from, to);

        let problems = check_project(root.path());

        assert_single_problem(&problems, "Cargo.toml", Rule::RustLintLevel, named, case);
    }
}

#[test]
fn check_passes_added_lints_and_raised_levels() {
    let root = complete_project();

    replace_in_manifest(root.path(), "todo = \"warn\"", "todo = \"deny\"");
    replace_in_manifest(
        root.path(),
        "unsafe_code = \"deny\"",
        "unsafe_code = { level = \"forbid\", priority = 0 }",
    );
    replace_in_manifest(
        root.path(),
        "dbg_macro = \"warn\"",
        "dbg_macro = { level = \"warn\" }\nunwrap_used = \"warn\"",
    );

    assert_eq!(check_project(root.path()), []);
}

#[test]
fn check_reports_an_unreadable_root_manifest() {
    let cases = [
        ("invalid TOML", "[workspace\n"),
        (
            "unknown level",
            "[workspace.lints.clippy]\ntodo = \"loud\"\n",
        ),
        (
            "level of the wrong type",
            "[workspace.lints.clippy]\ntodo = 3\n",
        ),
        (
            "members of the wrong type",
            "[workspace]\nmembers = \"crates/*\"\n",
        ),
    ];

    for (case, text) in cases {
        let root = complete_project();

        write_file(root.path(), "Cargo.toml", text);

        let problems = check_project(root.path());

        assert_single_problem(&problems, "Cargo.toml", Rule::RustConfigFile, "", case);
    }
}

#[test]
fn check_reports_a_missing_root_manifest() {
    let root = complete_project();

    fs::remove_file(root.path().join("Cargo.toml")).expect("remove Cargo.toml");

    let problems = check_project(root.path());

    assert_single_problem(&problems, "Cargo.toml", Rule::RustConfigFile, "", "missing");
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

#[test]
fn check_names_the_required_level_and_priority_of_a_missing_lint() {
    let root = complete_project();

    replace_in_manifest(
        root.path(),
        "all = { level = \"warn\", priority = -2 }\n",
        "",
    );

    let problems = check_project(root.path());

    assert_single_problem(
        &problems,
        "Cargo.toml",
        Rule::RustLintLevel,
        "`warn` with priority -2",
        "missing clippy::all",
    );
}
