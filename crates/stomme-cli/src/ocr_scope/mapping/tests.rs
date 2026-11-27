use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::ocr_scope::patch::parse_hunks;

#[test]
fn maps_a_moved_block_onto_its_new_position() {
    let patch = "@@ -1,4 +1,4 @@\n-alpha\n-beta\n gamma\n delta\n+alpha\n+beta";
    let covered = vec![
        covered_line("src/a.ts", 1, "alpha"),
        covered_line("src/a.ts", 2, "beta"),
    ];

    let mapped = map_covered_lines(
        &covered,
        &changes("src/a.ts", patch),
        &head_lines("src/a.ts", "gamma\ndelta\nalpha\nbeta\n"),
    );

    assert_eq!(
        mapped,
        [
            covered_line("src/a.ts", 3, "alpha"),
            covered_line("src/a.ts", 4, "beta"),
        ]
    );
    assert!(uncovered_added_lines("src/a.ts", &[3, 4], &claimed_positions(&mapped)).is_empty());
}

#[test]
fn maps_a_line_below_an_insertion_that_no_hunk_names() {
    let patch = "@@ -1 +1,2 @@\n-a\n+a\n+extra";
    let covered = vec![covered_line("src/a.ts", 3, "c")];

    let mapped = map_covered_lines(
        &covered,
        &changes("src/a.ts", patch),
        &head_lines("src/a.ts", "a\nextra\nb\nc\nd\n"),
    );

    assert_eq!(mapped, [covered_line("src/a.ts", 4, "c")]);
    assert_eq!(
        uncovered_added_lines("src/a.ts", &[1, 2, 4], &claimed_positions(&mapped)),
        BTreeSet::from([1, 2])
    );
}

#[test]
fn shifts_a_covered_line_up_when_a_line_above_it_is_deleted() {
    let patch = "@@ -1,3 +1,2 @@\n-a\n b\n c";
    let covered = vec![covered_line("src/a.ts", 3, "c")];

    let mapped = map_covered_lines(
        &covered,
        &changes("src/a.ts", patch),
        &head_lines("src/a.ts", "b\nc\n"),
    );

    assert_eq!(mapped, [covered_line("src/a.ts", 2, "c")]);
}

#[test]
fn keeps_a_covered_line_before_a_hunk_unshifted() {
    let patch = "@@ -2,2 +2,3 @@\n b\n+inserted\n c";
    let covered = vec![covered_line("src/a.ts", 1, "a")];

    let mapped = map_covered_lines(
        &covered,
        &changes("src/a.ts", patch),
        &head_lines("src/a.ts", "a\nb\ninserted\nc\n"),
    );

    assert_eq!(mapped, [covered_line("src/a.ts", 1, "a")]);
}

#[test]
fn counts_both_copies_as_new_when_a_removed_line_matches_two_added_copies() {
    let patch = "@@ -1,2 +1,3 @@\n-beta\n gamma\n+beta\n+beta";
    let covered = vec![covered_line("src/a.ts", 1, "beta")];

    let mapped = map_covered_lines(
        &covered,
        &changes("src/a.ts", patch),
        &head_lines("src/a.ts", "gamma\nbeta\nbeta\n"),
    );

    assert!(mapped.is_empty());
    assert_eq!(
        uncovered_added_lines("src/a.ts", &[2, 3], &claimed_positions(&mapped)),
        BTreeSet::from([2, 3])
    );
}

#[test]
fn gives_every_added_line_in_scope_when_there_is_no_coverage() {
    assert_eq!(
        uncovered_added_lines("src/a.ts", &[1, 2], &BTreeSet::new()),
        BTreeSet::from([1, 2])
    );
}

#[test]
fn maps_a_line_below_an_insertion_after_it() {
    let hunks = parse_hunks(b"@@ -82,0 +83 @@\n+inserted").expect("parse the hunk");

    assert_eq!(map_old_line(&hunks, 82), Some(82));
    assert_eq!(map_old_line(&hunks, 83), Some(84));
}

#[test]
fn maps_a_covered_line_at_the_last_old_line_of_a_hunk() {
    let patch = "@@ -2,2 +2,3 @@\n b\n+inserted\n c";
    let covered = vec![covered_line("src/a.ts", 3, "c")];

    let mapped = map_covered_lines(
        &covered,
        &changes("src/a.ts", patch),
        &head_lines("src/a.ts", "a\nb\ninserted\nc"),
    );

    assert_eq!(mapped, [covered_line("src/a.ts", 4, "c")]);
}

#[test]
fn keeps_a_covered_line_at_an_empty_old_range_unshifted() {
    let patch = "@@ -82,0 +83 @@\n+inserted";
    let head = (1..=82)
        .map(|number| format!("line {number}"))
        .collect::<Vec<_>>()
        .join("\n");
    let covered = vec![covered_line("src/a.ts", 82, "line 82")];

    let mapped = map_covered_lines(
        &covered,
        &changes("src/a.ts", patch),
        &head_lines("src/a.ts", &head),
    );

    assert_eq!(mapped, [covered_line("src/a.ts", 82, "line 82")]);
}

#[test]
fn shifts_a_covered_line_below_a_hunk_by_its_line_difference() {
    let patch = "@@ -2,3 +2,2 @@\n-b\n c\n d";
    let covered = vec![covered_line("src/a.ts", 5, "target")];

    let mapped = map_covered_lines(
        &covered,
        &changes("src/a.ts", patch),
        &head_lines("src/a.ts", "a\nc\nd\ntarget"),
    );

    assert_eq!(mapped, [covered_line("src/a.ts", 4, "target")]);
}

#[test]
fn inserts_a_removed_line_after_the_line_before_its_hunk() {
    let hunks = parse_hunks(b"@@ -2 +2,2 @@\n-old\n+new\n+extra").expect("parse the hunk");

    let inserted = scoped_removed_lines(&hunks, &BTreeSet::new());

    assert_eq!(
        inserted,
        [InsertedLine {
            after: 1,
            text: b"old".to_vec(),
            terminated: true,
        }]
    );
}

#[test]
fn inserts_removed_lines_at_a_deletion_hunk_start() {
    let hunks = parse_hunks(b"@@ -2,2 +1,0 @@\n-old\n-gone").expect("parse the hunk");

    let inserted = scoped_removed_lines(&hunks, &BTreeSet::new());

    assert_eq!(
        inserted,
        [
            InsertedLine {
                after: 1,
                text: b"old".to_vec(),
                terminated: true,
            },
            InsertedLine {
                after: 1,
                text: b"gone".to_vec(),
                terminated: true,
            },
        ]
    );
}

fn covered_line(path: &str, line: usize, text: &str) -> CoveredLine {
    CoveredLine {
        path: path.to_string(),
        line,
        text: text.as_bytes().to_vec(),
    }
}

fn changes(path: &str, patch: &str) -> BTreeMap<String, RangeChange> {
    let hunks = parse_hunks(patch.as_bytes());
    let change = RangeChange {
        new_path: path.to_string(),
        hunks,
    };

    BTreeMap::from([(path.to_string(), change)])
}

fn head_lines(path: &str, text: &str) -> BTreeMap<String, Vec<Vec<u8>>> {
    let lines = text
        .split('\n')
        .map(|line| line.as_bytes().to_vec())
        .collect();

    BTreeMap::from([(path.to_string(), lines)])
}
