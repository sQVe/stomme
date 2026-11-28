use super::*;

#[test]
fn reads_name_status_records() {
    let bytes = b"M\0src/modified.rs\0D\0src/deleted.rs\0R100\0src/old.rs\0src/new.rs\0";
    let cases = [
        (
            "a modification",
            0,
            'M',
            "src/modified.rs",
            "src/modified.rs",
        ),
        ("a deletion", 1, 'D', "src/deleted.rs", "src/deleted.rs"),
        ("a rename", 2, 'R', "src/old.rs", "src/new.rs"),
    ];

    let changes = parse_name_status(bytes).expect("parse the name-status records");

    assert_eq!(changes.len(), cases.len());

    for (name, index, status, old_path, new_path) in cases {
        assert_eq!(changes[index].status, status, "{name}");
        assert_eq!(changes[index].old_path, old_path, "{name}");
        assert_eq!(changes[index].new_path, new_path, "{name}");
    }
}

#[test]
fn reads_ordered_raw_records_with_rename_paths() {
    let bytes = b":100644 100644 old-object new-object M\0src/a.rs\0:100644 000000 old-object zero-object D\0src/b.rs\0:100644 100755 old-object new-object R100\0src/c.rs\0src/d.rs\0";

    let changes = parse_raw_changes(bytes).expect("parse the raw records");

    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].status, 'M');
    assert_eq!(changes[0].old_path, "src/a.rs");
    assert_eq!(changes[0].new_path, "src/a.rs");
    assert_eq!(changes[0].old_mode, "100644");
    assert_eq!(changes[0].new_mode, "100644");
    assert_eq!(changes[0].old_object, "old-object");
    assert_eq!(changes[1].status, 'D');
    assert_eq!(changes[1].old_path, "src/b.rs");
    assert_eq!(changes[2].status, 'R');
    assert_eq!(changes[2].old_path, "src/c.rs");
    assert_eq!(changes[2].new_path, "src/d.rs");
    assert_eq!(changes[2].new_mode, "100755");
}

#[test]
fn reads_binary_and_text_numstat_records() {
    let stats = parse_numstat(b"3\t4\tsrc/a.rs\0-\t-\tassets/logo.bin\0")
        .expect("parse the numstat records");

    assert!(matches!(
        stats.get("src/a.rs"),
        Some(Counts::Text {
            added: 3,
            removed: 4
        })
    ));
    assert!(matches!(stats.get("assets/logo.bin"), Some(Counts::Binary)));
}

#[test]
fn reads_a_rename_numstat_record_under_the_new_path() {
    let stats = parse_numstat(b"1\t2\t\0src/old.rs\0src/new.rs\0")
        .expect("parse the rename numstat record");

    assert!(matches!(
        stats.get("src/new.rs"),
        Some(Counts::Text {
            added: 1,
            removed: 2
        })
    ));
    assert!(!stats.contains_key("src/old.rs"));
}

#[test]
fn falls_back_to_zero_for_an_unparsed_numstat_count() {
    let stats = parse_numstat(b"not-a-number\t2\tsrc/a.rs\0").expect("parse the numstat record");

    assert!(matches!(
        stats.get("src/a.rs"),
        Some(Counts::Text {
            added: 0,
            removed: 2
        })
    ));
}
