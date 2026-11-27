use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[test]
fn counts_scoped_whole_files_and_line_files() {
    let mut added = BTreeSet::new();
    added.insert(1);
    added.insert(3);
    let removed = vec![InsertedLine {
        after: 1,
        text: "old".to_string(),
    }];
    let scoped = BTreeMap::from([
        (
            "src/lines.rs".to_string(),
            ScopedFile::Lines {
                added,
                removed,
                base_present: true,
            },
        ),
        (
            "src/whole.rs".to_string(),
            ScopedFile::Whole {
                base_object: None,
                base_mode: "100644".to_string(),
                added: 4,
                removed: 2,
            },
        ),
    ]);

    assert_eq!(counts(&scoped), (2, 6, 3));
}
