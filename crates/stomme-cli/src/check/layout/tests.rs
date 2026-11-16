use std::collections::BTreeSet;
use std::path::Path;

use super::*;
use crate::check::source_files;
use crate::layer::Language;
use crate::test_support::write_file;

fn problems(root: &Path, language: Language) -> Vec<Problem> {
    let tree = source_files::walk(root, language).expect("walk the project");

    match language {
        Language::Rust => rust::check(&tree),
        Language::TypeScript => {
            typescript::check(&tree, &BTreeSet::new()).expect("check the project")
        }
    }
}

#[test]
fn check_passes_unrelated_names() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    for file in [
        "src/synced_tree.rs",
        "src/sync.rs",
        "src/sync/record_state.rs",
        "src/gpui.rs",
        "src/gpui_component.rs",
    ] {
        write_file(root.path(), file, "");
    }

    write_file(root.path(), "vite.config.ts", "");

    assert_eq!(problems(root.path(), Language::Rust), Vec::<Problem>::new());
    assert_eq!(
        problems(root.path(), Language::TypeScript),
        Vec::<Problem>::new()
    );
}
