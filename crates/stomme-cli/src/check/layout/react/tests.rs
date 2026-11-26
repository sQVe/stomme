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
