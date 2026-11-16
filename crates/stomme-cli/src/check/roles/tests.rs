use std::fs;

use super::*;
use crate::rule::Rule;
use crate::test_support::write_file;

fn assert_single_problem(problems: &[Problem], path: &str, rule: Rule, named: &[&str], case: &str) {
    assert_eq!(problems.len(), 1, "{case}: {problems:?}");
    assert_eq!(problems[0].path, path, "{case}");
    assert_eq!(problems[0].rule, rule, "{case}");

    for value in named {
        assert!(
            problems[0].message.contains(value),
            "{case}: {value}: {}",
            problems[0].message
        );
    }
}

enum Setup {
    None,
    File,
    Folder,
}

struct Case {
    name: &'static str,
    role: &'static str,
    path: &'static str,
    setup: Setup,
    rule: Rule,
    problem_path: &'static str,
    named: &'static [&'static str],
}

#[test]
fn check_reports_each_broken_role() {
    let cases = [
        Case {
            name: "unknown role",
            role: "server",
            path: "crates/server",
            setup: Setup::None,
            rule: Rule::RoleName,
            problem_path: "stomme.toml",
            named: &["`server`", "`core`", "`cli`"],
        },
        Case {
            name: "missing path",
            role: "cli",
            path: "crates/tool",
            setup: Setup::None,
            rule: Rule::RolePath,
            problem_path: "stomme.toml",
            named: &["`cli`", "`crates/tool`"],
        },
        Case {
            name: "empty path",
            role: "cli",
            path: "",
            setup: Setup::None,
            rule: Rule::RolePath,
            problem_path: "stomme.toml",
            named: &["`cli`"],
        },
        Case {
            name: "root path",
            role: "cli",
            path: ".",
            setup: Setup::None,
            rule: Rule::RolePath,
            problem_path: "stomme.toml",
            named: &["`cli`", "`.`"],
        },
        Case {
            name: "parent part",
            role: "cli",
            path: "crates/../cli",
            setup: Setup::None,
            rule: Rule::RolePath,
            problem_path: "stomme.toml",
            named: &["`cli`", "`crates/../cli`"],
        },
        Case {
            name: "file instead of folder",
            role: "cli",
            path: "crates/cli",
            setup: Setup::File,
            rule: Rule::RolePath,
            problem_path: "stomme.toml",
            named: &["`cli`", "`crates/cli`"],
        },
        Case {
            name: "folder under another name",
            role: "cli",
            path: "crates/tool",
            setup: Setup::Folder,
            rule: Rule::RoleFolder,
            problem_path: "crates/tool",
            named: &["`cli`", "`crates/tool`", "-cli"],
        },
    ];

    for case in cases {
        let root = tempfile::tempdir().expect("create a temporary directory");
        let roles = BTreeMap::from([(case.role.to_string(), case.path.to_string())]);

        match case.setup {
            Setup::None => {}
            Setup::File => write_file(root.path(), case.path, ""),
            Setup::Folder => {
                fs::create_dir_all(root.path().join(case.path)).expect("create the role folder");
            }
        }

        let problems = check(root.path(), &roles);

        assert_single_problem(
            &problems,
            case.problem_path,
            case.rule,
            case.named,
            case.name,
        );
    }
}

#[cfg(unix)]
#[test]
fn check_reports_a_role_folder_symlinked_outside_the_project() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("create a temporary directory");
    let outside = tempfile::tempdir().expect("create an outside directory");
    let roles = BTreeMap::from([("core".to_string(), "src/core".to_string())]);

    fs::create_dir_all(root.path().join("src")).expect("create the source folder");
    fs::create_dir_all(outside.path().join("core")).expect("create the outside folder");
    symlink(outside.path().join("core"), root.path().join("src/core")).expect("create the symlink");

    let problems = check(root.path(), &roles);

    assert_single_problem(
        &problems,
        "stomme.toml",
        Rule::RolePath,
        &["`core`", "`src/core`", "outside the project"],
        "symlinked role folder",
    );
}

#[test]
fn check_accepts_role_folders_named_after_the_role() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let roles = BTreeMap::from([
        ("cli".to_string(), "crates/cli".to_string()),
        ("core".to_string(), "crates/tikk-core".to_string()),
        ("ui".to_string(), "src/ui".to_string()),
    ]);

    for path in ["crates/cli", "crates/tikk-core", "src/ui"] {
        fs::create_dir_all(root.path().join(path)).expect("create the role folder");
    }

    let problems = check(root.path(), &roles);

    assert_eq!(problems, Vec::new());
}

#[test]
fn two_roles_in_the_same_wrong_folder_get_distinct_keys() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let roles = BTreeMap::from([
        ("cli".to_string(), "src/domain".to_string()),
        ("ui".to_string(), "src/domain".to_string()),
    ]);

    fs::create_dir_all(root.path().join("src/domain")).expect("create the role folder");

    let problems = check(root.path(), &roles);
    let mut keys: Vec<&str> = problems
        .iter()
        .filter_map(|problem| problem.key.as_deref())
        .collect();

    keys.sort();

    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!(keys, ["cli:domain", "ui:domain"]);

    let recorded = vec![crate::config::Violation {
        path: "src/domain".to_string(),
        rule: Rule::RoleFolder,
        key: "cli:domain".to_string(),
    }];

    let remaining = crate::check::violations::apply(problems, &recorded);

    assert_eq!(remaining.len(), 1, "{remaining:?}");
    assert_eq!(remaining[0].key.as_deref(), Some("ui:domain"));
}
