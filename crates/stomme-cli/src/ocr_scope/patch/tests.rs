use super::*;

#[test]
fn parses_a_hunk_header_with_omitted_counts() {
    let hunks = parse_hunks(b"@@ -82,0 +83 @@\n+inserted").expect("parse the hunk");

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
                text: b"inserted".to_vec(),
                terminated: true,
            }],
        }]
    );
}

#[test]
fn rejects_a_bad_hunk_header() {
    assert_eq!(parse_hunks(b"@@ bad @@\n+inserted"), None);
}

#[test]
fn reads_removed_added_and_context_rows() {
    let hunks = parse_hunks(b"@@ -1,3 +1,2 @@\n-a\n b\n+c").expect("parse the hunk");

    assert_eq!(
        hunks[0].rows,
        [
            Row {
                kind: RowKind::Removed,
                old_line: Some(1),
                new_line: None,
                text: b"a".to_vec(),
                terminated: true,
            },
            Row {
                kind: RowKind::Context,
                old_line: Some(2),
                new_line: Some(1),
                text: b"b".to_vec(),
                terminated: true,
            },
            Row {
                kind: RowKind::Added,
                old_line: None,
                new_line: Some(2),
                text: b"c".to_vec(),
                terminated: true,
            },
        ]
    );
}

#[test]
fn stops_a_hunk_at_the_counts_its_header_declares() {
    let patch = b"@@ -1 +0,0 @@\n-target\n--- /dev/null\n+++ b/src/a.txt\n@@ -0,0 +1 @@\n+new";

    let hunks = parse_hunks(patch).expect("parse the hunks");

    assert_eq!(hunks.len(), 2);
    assert_eq!(
        hunks[0].rows,
        [Row {
            kind: RowKind::Removed,
            old_line: Some(1),
            new_line: None,
            text: b"target".to_vec(),
            terminated: true,
        }]
    );
    assert_eq!(
        hunks[1].rows,
        [Row {
            kind: RowKind::Added,
            old_line: None,
            new_line: Some(1),
            text: b"new".to_vec(),
            terminated: true,
        }]
    );
}

#[test]
fn ignores_a_row_beyond_the_counts_its_hunk_declares() {
    let cases: [RowCase; 6] = [
        (
            "a removed row after the declared removals",
            b"@@ -1,2 +0,0 @@\n-a\n-b\n-c",
            &[b"a", b"b"],
        ),
        (
            "an added row after the declared additions",
            b"@@ -1,0 +1,2 @@\n+a\n+b\n+c",
            &[b"a", b"b"],
        ),
        (
            "a removed row without an old line left",
            b"@@ -1,0 +1,2 @@\n+new\n-stray\n+second",
            &[b"new", b"second"],
        ),
        (
            "an added row without a new line left",
            b"@@ -1,2 +1,0 @@\n-a\n+stray\n-b",
            &[b"a", b"b"],
        ),
        (
            "a context row without an old line left",
            b"@@ -1,0 +1,2 @@\n+new\n keep",
            &[b"new"],
        ),
        (
            "a context row without a new line left",
            b"@@ -1,2 +1,0 @@\n-a\n keep",
            &[b"a"],
        ),
    ];

    for (name, patch, expected) in cases {
        let hunks = parse_hunks(patch).expect("parse the hunk");
        let texts: Vec<&[u8]> = hunks[0]
            .rows
            .iter()
            .map(|row| row.text.as_slice())
            .collect();

        assert_eq!(texts, expected, "{name}");
    }
}

type RowCase = (&'static str, &'static [u8], &'static [&'static [u8]]);

#[test]
fn keeps_the_bytes_of_a_non_utf8_row() {
    let hunks = parse_hunks(b"@@ -1 +0,0 @@\n-latin-1 \xe9").expect("parse the hunk");

    assert_eq!(hunks[0].rows[0].text, b"latin-1 \xe9");
}

#[test]
fn marks_a_row_before_the_no_newline_marker_as_unterminated() {
    let hunks = parse_hunks(b"@@ -2,2 +1,1 @@\n keep\n-last\n\\ No newline at end of file\n")
        .expect("parse the hunk");

    assert!(hunks[0].rows[0].terminated);
    assert!(!hunks[0].rows[1].terminated);
}
