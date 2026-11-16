use std::collections::BTreeSet;
use std::path::Path;

use super::*;
use crate::check::layout::source_files;
use crate::check::layout::typescript;
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
