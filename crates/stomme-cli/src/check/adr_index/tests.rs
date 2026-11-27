use std::fs;
use std::path::PathBuf;

use super::*;
use crate::rule::Rule;
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

    assert_eq!(check(root.path(), "docs/adr").expect("read the ADRs"), []);
}

#[test]
fn check_skips_a_project_without_an_index() {
    let root = complete_project();

    fs::remove_file(root.path().join(INDEX)).expect("remove the index");
    write_file(root.path(), ADR, "# Not an ADR heading\n");

    assert_eq!(check(root.path(), "docs/adr").expect("read the ADRs"), []);
}

#[test]
fn check_ignores_files_that_are_not_numbered_adrs() {
    let names = ["abcd-title.md", "0001-scope.txt", "x", "猫猫.md", "abcd.md"];

    for name in names {
        let root = complete_project();

        write_file(root.path(), &format!("docs/adr/{name}"), "No heading\n");

        assert_eq!(
            check(root.path(), "docs/adr").expect("read the ADRs"),
            [],
            "{name}"
        );
    }
}

#[test]
fn check_ignores_a_directory_named_like_an_adr_file() {
    for name in ["0001-.md", "0002-other.md"] {
        let root = complete_project();

        fs::create_dir(root.path().join(format!("docs/adr/{name}"))).expect("create the directory");

        assert_eq!(
            check(root.path(), "docs/adr").expect("read the ADRs"),
            [],
            "{name}"
        );
    }
}

#[test]
fn check_reports_two_files_with_one_number() {
    let root = complete_project();

    write_file(root.path(), "docs/adr/0001-other.md", "# ADR 0001: Other\n");
    add_index_line(root.path(), "- [0001: Other](./0001-other.md)");

    let problems = check(root.path(), "docs/adr").expect("read the ADRs");

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

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

        assert_eq!(found(&problems), [(ADR, Rule::AdrHeading)], "{heading}");
    }
}

#[test]
fn check_accepts_a_heading_after_text_and_with_up_to_three_spaces() {
    let cases = [
        ("no indent", "# ADR 0001: Documentation scope"),
        ("two spaces", "  # ADR 0001: Documentation scope  "),
        ("three spaces", "   # ADR 0001: Documentation scope"),
        (
            "after text",
            "Draft notes\n\n# ADR 0001: Documentation scope",
        ),
        (
            "hashtag before the heading",
            "#hashtag\n\n# ADR 0001: Documentation scope",
        ),
        (
            "backtick fence",
            "```md\n# ADR 0001: Other\n```\n\n# ADR 0001: Documentation scope",
        ),
        (
            "short fence run",
            "```md\n``\n# ADR 0001: Other\n```\n# ADR 0001: Documentation scope",
        ),
        ("short backticks", "``\n# ADR 0001: Documentation scope"),
        (
            "backtick in a backtick info string",
            "``` `code`\n# ADR 0001: Documentation scope",
        ),
        (
            "backticks in a tilde info string",
            "~~~ `code`\n# ADR 0001: Other\n~~~\n# ADR 0001: Documentation scope",
        ),
        (
            "tilde fence",
            "~~~\n# ADR 0001: Other\n~~~\n\n# ADR 0001: Documentation scope",
        ),
    ];

    for (case, text) in cases {
        let root = complete_project();

        write_file(root.path(), ADR, &format!("{text}\n"));

        assert_eq!(
            check(root.path(), "docs/adr").expect("read the ADRs"),
            [],
            "{case}"
        );
    }
}

#[test]
fn check_reports_a_heading_that_is_a_code_block() {
    let cases = [
        ("four spaces", "    # ADR 0001: Documentation scope"),
        ("tab", "\t# ADR 0001: Documentation scope"),
    ];

    for (case, text) in cases {
        let root = complete_project();

        write_file(root.path(), ADR, &format!("{text}\n"));

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

        assert_eq!(found(&problems), [(ADR, Rule::AdrHeading)], "{case}");
    }
}

#[test]
fn check_reads_a_heading_past_a_fence_inside_a_list_item() {
    let cases = [
        (
            "bullet with a deeper fence",
            "- Topic\n\n     ```md\n  # ADR 0001: Other\n     ```\n\n# ADR 0001: Documentation scope",
        ),
        (
            "ordered list with a deeper fence",
            "1. Topic\n\n    ```md\n   # ADR 0001: Other\n    ```\n\n# ADR 0001: Documentation scope",
        ),
        (
            "ordered list with a fence at the content column",
            "1. Topic\n\n   ```md\n   # ADR 0001: Other\n   ```\n\n# ADR 0001: Documentation scope",
        ),
    ];

    for (case, text) in cases {
        let root = complete_project();

        write_file(root.path(), ADR, &format!("{text}\n"));

        assert_eq!(
            check(root.path(), "docs/adr").expect("read the ADRs"),
            [],
            "{case}"
        );
    }
}

#[test]
fn check_reports_a_missing_heading_when_only_a_fence_inside_a_list_item_has_one() {
    let cases = [
        (
            "bullet",
            "- Topic\n\n     ```md\n  # ADR 0001: Documentation scope\n     ```",
        ),
        (
            "ordered list",
            "1. Topic\n\n    ```md\n   # ADR 0001: Documentation scope\n    ```",
        ),
    ];

    for (case, text) in cases {
        let root = complete_project();

        write_file(root.path(), ADR, &format!("{text}\n"));

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

        assert_eq!(found(&problems), [(ADR, Rule::AdrHeading)], "{case}");
    }
}

#[test]
fn check_reads_a_heading_after_a_fence_that_ends_with_its_list_item() {
    let root = complete_project();

    write_file(
        root.path(),
        ADR,
        "1. Topic\n\n    ```md\n# ADR 0001: Other\n    ```\n\n# ADR 0001: Documentation scope\n",
    );

    let problems = check(root.path(), "docs/adr").expect("read the ADRs");

    assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexLabel)]);
    assert!(problems[0].message.contains("`0001: Other`"));
}

#[test]
fn check_reports_a_numbered_adr_file_without_a_slug() {
    let cases = ["0001-.md", "0001.md"];

    for name in cases {
        let root = complete_project();
        let path = format!("docs/adr/{name}");

        write_file(root.path(), &path, "# ADR 0001: Documentation scope\n");

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

        assert_eq!(
            found(&problems),
            [(path.as_str(), Rule::AdrFileName)],
            "{name}"
        );

        let message = &problems[0].message;

        assert!(message.contains(name), "{name}: {message}");
        assert!(message.contains("NNNN-slug.md"), "{name}: {message}");
    }
}

#[test]
fn check_reports_files_without_a_slug_sorted_by_path() {
    let creation_orders = [
        [
            "0007-.md", "0003.md", "0009-.md", "0002-.md", "0005.md", "0004-.md",
        ],
        [
            "0002-.md", "0003.md", "0004-.md", "0005.md", "0007-.md", "0009-.md",
        ],
        [
            "0009-.md", "0007-.md", "0005.md", "0004-.md", "0003.md", "0002-.md",
        ],
    ];
    let expected = [
        "docs/adr/0002-.md",
        "docs/adr/0003.md",
        "docs/adr/0004-.md",
        "docs/adr/0005.md",
        "docs/adr/0007-.md",
        "docs/adr/0009-.md",
    ];

    for names in creation_orders {
        let root = complete_project();

        for name in names {
            write_file(root.path(), &format!("docs/adr/{name}"), "");
        }

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");
        let paths: Vec<&str> = problems
            .iter()
            .map(|problem| problem.path.as_str())
            .collect();

        assert_eq!(paths, expected, "created in the order {names:?}");
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

    let problems = check(root.path(), "docs/adr").expect("read the ADRs");

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

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

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

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

        assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexLabel)], "{label}");
    }
}

#[test]
fn check_groups_index_entries_by_file() {
    let root = complete_project();

    write_file(root.path(), "docs/adr/0003-three.md", "# ADR 0003: Three\n");
    replace_in_index(
        root.path(),
        ENTRY,
        &format!(
            "{ENTRY}- [0001: Documentation scope](./0001-documentation-scope.md)\n- [0003: Three](./0003-three.md)\n- [0009: Gone](./0009-gone.md)\n"
        ),
    );

    let problems = check(root.path(), "docs/adr").expect("read the ADRs");

    assert_eq!(
        found(&problems),
        [
            (INDEX, Rule::AdrIndexCount),
            (INDEX, Rule::AdrIndexMissingFile),
        ]
    );
}

#[test]
fn check_reports_an_index_entry_for_a_missing_file() {
    let root = complete_project();

    add_index_line(root.path(), "- [0009: Gone](./0009-gone.md)");

    let problems = check(root.path(), "docs/adr").expect("read the ADRs");

    assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexMissingFile)]);
}

#[test]
fn check_accepts_each_bullet() {
    let cases = [("dash", "-"), ("star", "*"), ("plus", "+")];

    for (case, bullet) in cases {
        let root = complete_project();

        replace_in_index(root.path(), "- [0001", &format!("{bullet} [0001"));

        assert_eq!(
            check(root.path(), "docs/adr").expect("read the ADRs"),
            [],
            "{case}"
        );
    }
}

#[test]
fn check_accepts_a_nested_indented_index_entry() {
    let cases = [
        (
            "two spaces",
            "  - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "four spaces",
            "- Topic\n    - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "tab",
            "- Topic\n\t- [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "after a blank line",
            "- Topic\n\n    - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "after an indented continuation",
            "- Topic\n\n  Details\n\n    - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "three spaces after the bullet",
            "-   Topic\n\n      - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "four spaces after the bullet",
            "-    Topic\n\n       - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "two spaces and a tab",
            "- Topic\n\n  \t- [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "nine-digit ordered marker",
            "123456789. Topic\n\n           - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
    ];

    for (case, replacement) in cases {
        let root = complete_project();

        replace_in_index(root.path(), ENTRY.trim_end(), replacement);

        assert_eq!(
            check(root.path(), "docs/adr").expect("read the ADRs"),
            [],
            "{case}"
        );
    }
}

#[test]
fn check_reports_a_standalone_indented_index_link_as_not_listed() {
    let cases = [
        (
            "four spaces at the start of the section",
            "    - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "tab at the start of the section",
            "\t- [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "after plain text",
            "Plain text\n    - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "after a code block",
            "    - Topic\n    - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
    ];

    for (case, replacement) in cases {
        let root = complete_project();

        replace_in_index(root.path(), ENTRY.trim_end(), replacement);

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

        assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexCount)], "{case}");
    }
}

#[test]
fn check_reports_an_index_link_inside_indented_code_in_a_list() {
    let cases = [
        (
            "four spaces below a list item",
            "- Topic\n\n      - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "four spaces below a heading that ends the list",
            "- Topic\n\n ### Group\n    - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "five spaces after the bullet",
            "-     Topic\n\n      - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
        (
            "below a blank list item with three spaces",
            "-   \n      - [0001: Documentation scope](./0001-documentation-scope.md)",
        ),
    ];

    for (case, replacement) in cases {
        let root = complete_project();

        replace_in_index(root.path(), ENTRY.trim_end(), replacement);

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

        assert_eq!(found(&problems), [(INDEX, Rule::AdrIndexCount)], "{case}");
    }
}

#[test]
fn check_ignores_an_index_link_inside_a_fenced_block() {
    let cases = [
        (
            "fence on a list marker line",
            "- ```\n  - [0001: Documentation scope](./0001-documentation-scope.md)\n  ```\n",
        ),
        (
            "fence after two list markers",
            "- - ```\n    - [0001: Documentation scope](./0001-documentation-scope.md)\n    ```\n",
        ),
        (
            "fence in a quoted list item",
            "> - ```\n>   - [0001: Documentation scope](./0001-documentation-scope.md)\n>   ```\n",
        ),
        (
            "fence in the index",
            "```\n- [0001: Documentation scope](./0001-documentation-scope.md)\n```\n",
        ),
    ];

    for (case, sample) in cases {
        let root = complete_project();

        replace_in_index(root.path(), ENTRY, &format!("{sample}{ENTRY}"));

        assert_eq!(
            check(root.path(), "docs/adr").expect("read the ADRs"),
            [],
            "{case}"
        );
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
        "- [Template](./TEMPLATE.md)",
        "- [Notes](./notes.md)",
        "\u{a0}- [0001: Documentation scope](./0001-documentation-scope.md)",
    ];

    for line in cases {
        let root = complete_project();

        add_index_line(root.path(), line);

        let problems = check(root.path(), "docs/adr").expect("read the ADRs");

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

    assert_eq!(check(root.path(), "docs/adr").expect("read the ADRs"), []);
}

#[test]
fn check_reports_every_problem_in_one_run() {
    let root = complete_project();

    write_file(root.path(), ADR, "# Documentation scope\n");
    add_index_line(root.path(), "- [0009: Gone](./0009-gone.md)");

    let problems = check(root.path(), "docs/adr").expect("read the ADRs");

    assert_eq!(
        found(&problems),
        [(ADR, Rule::AdrHeading), (INDEX, Rule::AdrIndexMissingFile),]
    );
}

const SHARED_FOLDER: &str = "shared/adr";

fn foundation_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the stomme crate lives two levels below the repository root")
        .join("foundation")
}

fn copy_shared_adrs(target: &Path) {
    let source = foundation_root().join(SHARED_FOLDER);
    let entries = fs::read_dir(&source).expect("list the shared ADRs");

    for entry in entries {
        let entry = entry.expect("read a shared ADR entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let text = fs::read_to_string(entry.path()).expect("read a shared ADR");

        write_file(target, &format!("{SHARED_FOLDER}/{name}"), &text);
    }
}

#[test]
fn check_passes_the_shared_adrs() {
    let root = foundation_root();

    assert!(root.join(SHARED_FOLDER).join("README.md").is_file());
    assert_eq!(check(&root, SHARED_FOLDER).expect("read the ADRs"), []);
}

#[test]
fn check_reports_a_shared_adr_left_out_of_its_index() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let index = format!("{SHARED_FOLDER}/README.md");

    copy_shared_adrs(root.path());

    let text = fs::read_to_string(root.path().join(&index)).expect("read the shared index");
    let entry = text
        .lines()
        .find(|line| line.starts_with("- [0001: "))
        .expect("the shared index lists ADR 0001");
    let without_entry = text.replace(&format!("{entry}\n"), "");

    write_file(root.path(), &index, &without_entry);

    let problems = check(root.path(), SHARED_FOLDER).expect("read the ADRs");

    assert_eq!(found(&problems), [(index.as_str(), Rule::AdrIndexCount)]);
}
