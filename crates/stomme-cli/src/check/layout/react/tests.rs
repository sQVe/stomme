use std::collections::BTreeSet;
use std::path::Path;

use super::*;
use crate::check::layout::typescript;
use crate::check::source_files;
use crate::layer::Language;
use crate::rule::Rule;
use crate::test_support::write_file;

fn tree(root: &Path) -> Tree {
    source_files::walk(root, Language::TypeScript).expect("walk the project")
}

fn rules(problems: &[Problem]) -> Vec<Rule> {
    problems.iter().map(|problem| problem.rule).collect()
}

#[test]
fn tsx_test_placement_accepts_an_owner_of_either_extension() {
    let cases = [
        (
            "component owner",
            &["src/TicketList.tsx"][..],
            "src/TicketList.test.tsx",
            None,
        ),
        (
            "module owner",
            &["src/report.ts"][..],
            "src/report.test.tsx",
            None,
        ),
        (
            "test sibling only",
            &["src/report.test.ts"][..],
            "src/report.test.tsx",
            Some(Rule::ReactTestPlacement),
        ),
        (
            "one letter family stem",
            &["src/TicketList.tsx"][..],
            "src/TicketListX.test.tsx",
            Some(Rule::ReactTestPlacement),
        ),
        (
            "multi-word family suffix",
            &["src/Button.tsx"][..],
            "src/ButtonClick.extra.test.tsx",
            Some(Rule::ReactTestPlacement),
        ),
        (
            "pascal case family suffix",
            &["src/Button.tsx"][..],
            "src/ButtonClick.test.tsx",
            None,
        ),
        (
            "no owner",
            &[][..],
            "src/missing.test.tsx",
            Some(Rule::ReactTestPlacement),
        ),
    ];

    for (case, owners, test, expected) in cases {
        let root = tempfile::tempdir().expect("create a temporary directory");

        for owner in owners {
            write_file(root.path(), owner, "");
        }

        write_file(root.path(), test, "");

        let problems = check(&tree(root.path()));
        let expected_rules: Vec<Rule> = expected.into_iter().collect();

        assert_eq!(rules(&problems), expected_rules, "{case}");
        assert!(
            problems.iter().all(|problem| problem.path == test),
            "{case}: {problems:?}"
        );
    }
}

#[test]
fn tsx_placement_reports_a_tests_folder() {
    let cases = [
        (
            "test file",
            "src/__tests__/Ticket.test.tsx",
            "Ticket.test.tsx",
            "put it beside its owner",
        ),
        (
            "component file",
            "src/__tests__/Ticket.tsx",
            "Ticket.tsx",
            "move it out of the tests folder",
        ),
    ];

    for (case, path, name, advice) in cases {
        let root = tempfile::tempdir().expect("create a temporary directory");

        write_file(root.path(), path, "");

        let problems = check(&tree(root.path()));
        let named = format!("puts `{name}` in a `__tests__` folder");

        assert_eq!(problems.len(), 1, "{case}: {problems:?}");
        assert_eq!(problems[0].path, path, "{case}");
        assert_eq!(problems[0].rule, Rule::ReactTestPlacement, "{case}");
        assert!(problems[0].message.contains(&named), "{case}: {problems:?}");
        assert!(problems[0].message.contains(advice), "{case}: {problems:?}");
    }
}

#[test]
fn tsx_placement_ignores_a_tests_folder_outside_src() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), "e2e/__tests__/login.test.tsx", "");

    assert_eq!(rules(&check(&tree(root.path()))), Vec::<Rule>::new());
}

#[test]
fn tsx_placement_rejects_an_all_uppercase_component_stem() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    write_file(root.path(), "src/TICKETLIST.tsx", "");

    let problems = check(&tree(root.path()));

    assert_eq!(rules(&problems), [Rule::ReactComponentFile]);
}

#[test]
fn tsx_rules_run_only_with_the_react_layer() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    write_file(root.path(), "src/button.tsx", "");
    let project = tree(root.path());

    assert_eq!(rules(&check(&project)), [Rule::ReactComponentFile]);
    assert_eq!(
        rules(&typescript::check(&project, &BTreeSet::new()).expect("check the project")),
        Vec::<Rule>::new()
    );

    let root = tempfile::tempdir().expect("create a temporary directory");
    write_file(root.path(), "src/Button.tsx", "");
    let project = tree(root.path());

    assert_eq!(rules(&check(&project)), Vec::<Rule>::new());
    assert_eq!(
        rules(&typescript::check(&project, &BTreeSet::new()).expect("check the project")),
        [Rule::TypeScriptFileCase]
    );
}
