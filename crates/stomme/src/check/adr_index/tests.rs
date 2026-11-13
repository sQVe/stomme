use std::fs;

use super::*;
use crate::check::Rule;
use crate::test_support::{complete_project, write_file};

const INDEX: &str = "docs/adr/README.md";
const ADR: &str = "docs/adr/0001-documentation-scope.md";
const ENTRY: &str = "- [0001: Documentation scope](./0001-documentation-scope.md)\n";

fn found(problems: &[Problem]) -> Vec<(&str, Rule)> {
    problems
        .iter()
        .map(|problem| (problem.path.as_str(), problem.rule))
        .collect()
}

fn index_text(root: &Path) -> String {
    fs::read_to_string(root.join(INDEX)).expect("read the fixture index")
}

fn replace_in_index(root: &Path, from: &str, to: &str) {
    let text = index_text(root);

    assert!(text.contains(from), "the fixture index contains {from:?}");
    write_file(root, INDEX, &text.replace(from, to));
}

fn add_index_line(root: &Path, line: &str) {
    replace_in_index(root, ENTRY, &format!("{ENTRY}{line}\n"));
}

#[test]
fn check_passes_a_consistent_adr_directory() {
    let root = complete_project();

    assert_eq!(check(root.path()).expect("read the ADRs"), []);
}

#[test]
fn check_skips_a_project_without_an_index() {
    let root = complete_project();

    fs::remove_file(root.path().join(INDEX)).expect("remove the index");
    write_file(root.path(), ADR, "# Not an ADR heading\n");

    assert_eq!(check(root.path()).expect("read the ADRs"), []);
}

#[test]
fn check_reports_two_files_with_one_number() {
    let root = complete_project();

    write_file(root.path(), "docs/adr/0001-other.md", "# ADR 0001: Other\n");
    add_index_line(root.path(), "- [0001: Other](./0001-other.md)");

    let problems = check(root.path()).expect("read the ADRs");

    assert_eq!(
        found(&problems),
        [
            (ADR, Rule::AdrDuplicateNumber),
            ("docs/adr/0001-other.md", Rule::AdrDuplicateNumber),
        ]
    );
}

#[test]
fn check_reports_a_first_heading_that_does_not_match_the_file() {
    let cases = [
        "# ADR 0002: Documentation scope",
        "# Documentation scope",
        "# ADR 0001:",
        "## ADR 0001: Documentation scope",
        "Documentation scope",
        "  # Draft\n\n# ADR 0001: Documentation scope",
    ];

    for heading in cases {
        let root = complete_project();

        write_file(root.path(), ADR, &format!("{heading}\n\nText.\n"));

        let problems = check(root.path()).expect("read the ADRs");

        assert_eq!(found(&problems), [(ADR, Rule::AdrHeading)], "{heading}");
    }
}

#[test]
fn check_accepts_a_heading_after_text_and_with_indentation() {
    let cases = [
        "Draft notes\n\n# ADR 0001: Documentation scope",
        "  # ADR 0001: Documentation scope  ",
    ];

    for text in cases {
        let root = complete_project();

        write_file(root.path(), ADR, &format!("{text}\n"));

        assert_eq!(check(root.path()).expect("read the ADRs"), [], "{text}");
    }
}

#[test]
fn check_reports_an_adr_that_reuses_a_retired_number() {
    let root = complete_project();

    write_file(
        root.path(),
        "docs/adr/0002-reused.md",
        "# ADR 0002: Reused\n",
    );
    add_index_line(root.path(), "- [0002: Reused](./0002-reused.md)");

    let problems = check(root.path()).expect("read the ADRs");

    assert_eq!(
        found(&problems),
        [("docs/adr/0002-reused.md", Rule::AdrRetiredNumber)]
    );
}

#[test]
fn check_reports_an_adr_listed_zero_times_or_twice() {
    let doubled = format!("{ENTRY}{ENTRY}");
    let cases = [("missing", ""), ("doubled", doubled.as_str())];

    for (case, entries) in cases {
        let root = complete_project();

        replace_in_index(root.path(), ENTRY, entries);

        let problems = check(root.path()).expect("read the ADRs");

        assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexCount)], "{case}");
    }
}

#[test]
fn check_reports_a_label_that_does_not_match_the_heading() {
    let cases = [
        "0001: Documentation",
        "Documentation scope",
        "0002: Documentation scope",
    ];

    for label in cases {
        let root = complete_project();

        replace_in_index(
            root.path(),
            "[0001: Documentation scope]",
            &format!("[{label}]"),
        );

        let problems = check(root.path()).expect("read the ADRs");

        assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexLabel)], "{label}");
    }
}

#[test]
fn check_reports_an_index_entry_for_a_missing_file() {
    let root = complete_project();

    add_index_line(root.path(), "- [0009: Gone](./0009-gone.md)");

    let problems = check(root.path()).expect("read the ADRs");

    assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexMissingFile)]);
}

#[test]
fn check_reads_star_and_plus_bullets() {
    for bullet in ["*", "+"] {
        let root = complete_project();

        replace_in_index(root.path(), "- [0001", &format!("{bullet} [0001"));

        assert_eq!(check(root.path()).expect("read the ADRs"), [], "{bullet}");
    }
}

#[test]
fn check_reports_an_index_line_that_is_not_a_plain_entry() {
    let cases = [
        "- [0001: Documentation scope](./0001-documentation-scope.md) (see also 0002)",
        "- See [0001: Documentation scope](./0001-documentation-scope.md)",
        "-[0001: Documentation scope](./0001-documentation-scope.md)",
        "- [0001: Documentation scope](../adr/0001-documentation-scope.md)",
        "- [0001: Documentation scope][scope]",
        "[scope]: ./0001-documentation-scope.md",
        "- [scope]",
        "- <./0001-documentation-scope.md>",
        "- 0001-documentation-scope.md",
    ];

    for line in cases {
        let root = complete_project();

        add_index_line(root.path(), line);

        let problems = check(root.path()).expect("read the ADRs");

        assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexEntry)], "{line}");
    }
}

#[test]
fn check_reads_only_the_index_section_for_entries() {
    let root = complete_project();

    replace_in_index(
        root.path(),
        "Follow the template for every new ADR.",
        "Follow [the template](./TEMPLATE.md) and [ADR 0001][scope].",
    );

    assert_eq!(check(root.path()).expect("read the ADRs"), []);
}

#[test]
fn check_reports_every_problem_in_one_run() {
    let root = complete_project();

    write_file(root.path(), ADR, "# Documentation scope\n");
    add_index_line(root.path(), "- [0009: Gone](./0009-gone.md)");

    let problems = check(root.path()).expect("read the ADRs");

    assert_eq!(
        found(&problems),
        [(ADR, Rule::AdrHeading), (INDEX, Rule::AdrIndexMissingFile),]
    );
}
