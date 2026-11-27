use super::*;

#[test]
fn parses_a_hunk_header_with_omitted_counts() {
    let hunks = parse_hunks("@@ -82,0 +83 @@\n+inserted").expect("parse the hunk");

    assert_eq!(
        hunks,
        [Hunk {
            old_start: 82,
            old_count: 0,
            new_start: 83,
            new_count: 1,
            rows: vec![Row {
                kind: RowKind::Added,
                old_line: None,
                new_line: Some(83),
                text: "inserted".to_string(),
            }],
        }]
    );
}

#[test]
fn rejects_a_bad_hunk_header() {
    assert_eq!(parse_hunks("@@ bad @@\n+inserted"), None);
}

#[test]
fn reads_removed_added_and_context_rows() {
    let hunks = parse_hunks("@@ -1,3 +1,2 @@\n-a\n b\n+c").expect("parse the hunk");

    assert_eq!(
        hunks[0].rows,
        [
            Row {
                kind: RowKind::Removed,
                old_line: Some(1),
                new_line: None,
                text: "a".to_string(),
            },
            Row {
                kind: RowKind::Context,
                old_line: Some(2),
                new_line: Some(1),
                text: "b".to_string(),
            },
            Row {
                kind: RowKind::Added,
                old_line: None,
                new_line: Some(2),
                text: "c".to_string(),
            },
        ]
    );
}
