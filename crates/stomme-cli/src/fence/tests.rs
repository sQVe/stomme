use super::*;

fn scanned(text: &str) -> Vec<(usize, usize, &str)> {
    unfenced_lines(text.lines())
        .map(|line| (line.index, line.view.relative, line.view.content))
        .collect()
}

#[test]
fn a_fence_hides_the_lines_until_its_closer() {
    let cases = [
        ("backtick", "```md", "```"),
        ("tilde", "~~~md", "~~~"),
        ("four backticks", "````", "````"),
        ("three spaces of indent", "   ```", "   ```"),
    ];

    for (case, opener, closer) in cases {
        let text = format!("a\n{opener}\nhidden\n{closer}\nb\n");

        assert_eq!(scanned(&text), [(0, 0, "a"), (4, 0, "b")], "{case}");
    }
}

#[test]
fn a_fence_needs_its_marker_and_its_length_to_close() {
    let cases = [
        ("another marker", "```", "~~~"),
        ("a shorter run", "````", "```"),
        ("trailing text", "```", "``` not a closer"),
    ];

    for (case, opener, closer) in cases {
        let text = format!("{opener}\nhidden\n{closer}\nstill hidden\n");
        let lines = scanned(&text);

        assert!(lines.is_empty(), "{case}: {lines:?}");
    }
}

#[test]
fn a_fence_closes_only_on_its_own_marker_at_least_as_long() {
    for marker in ['`', '~'] {
        for length in 3..=5 {
            for indent in 0..=3 {
                let pad = " ".repeat(indent);
                let opener = format!("{pad}{}", marker.to_string().repeat(length));
                let closer = format!("{pad}{}", marker.to_string().repeat(length));
                let text = format!("{opener}\nhidden\n{closer}\nafter\n");

                assert_eq!(scanned(&text), [(3, 0, "after")], "{opener:?}");

                let shorter = format!("{pad}{}", marker.to_string().repeat(length - 1));
                let text = format!("{opener}\nhidden\n{shorter}\nstill hidden\n");
                let lines = scanned(&text);

                assert!(
                    lines.is_empty(),
                    "{opener:?} closes on {shorter:?}: {lines:?}"
                );
            }
        }
    }
}

#[test]
fn four_spaces_of_indent_is_code_not_a_fence() {
    let text = "    ```\nhidden\n    ```\na\n";

    assert_eq!(scanned(text), [(1, 0, "hidden"), (3, 0, "a")]);
}

#[test]
fn a_list_item_sets_the_column_its_fence_is_measured_from() {
    let text = "- Topic\n     ```md\n  inside\n     ```\n# Heading\n";

    assert_eq!(scanned(text), [(0, 0, "- Topic"), (4, 0, "# Heading")]);
}

#[test]
fn a_fence_inside_a_list_item_ends_with_the_item() {
    let text = "1. Topic\n    ```md\n# Heading\n    ```\n";

    assert_eq!(scanned(text), [(0, 0, "1. Topic"), (2, 0, "# Heading")]);
}

#[test]
fn a_fence_on_a_list_marker_line_hides_the_item_content() {
    let cases = [
        ("bullet", "- ```\n  - [entry](./0001-a.md)\n  ```\nafter\n"),
        ("star", "* ```\n  - [entry](./0001-a.md)\n  ```\nafter\n"),
        (
            "ordered",
            "1. ```\n   - [entry](./0001-a.md)\n   ```\nafter\n",
        ),
        (
            "two markers",
            "- - ```\n    - [entry](./0001-a.md)\n    ```\nafter\n",
        ),
    ];

    for (case, text) in cases {
        assert_eq!(scanned(text), [(3, 0, "after")], "{case}");
    }
}

#[test]
fn a_quote_inside_a_fence_is_content() {
    let text = "- Topic\n  ```\n  > quoted\n  ```\nafter\n";

    assert_eq!(scanned(text), [(0, 0, "- Topic"), (4, 0, "after")]);
}

#[test]
fn a_quoted_fence_ends_with_the_quote() {
    let text = "> ```\n> hidden\nafter\n";

    assert_eq!(scanned(text), [(2, 0, "after")]);
}

#[test]
fn a_quote_inside_a_fence_does_not_close_it() {
    let text = "- Topic\n  ```\n  > ```\n  still hidden\nafter\n";

    assert_eq!(scanned(text), [(0, 0, "- Topic"), (4, 0, "after")]);
}

#[test]
fn a_fence_that_leaves_its_item_starts_a_quoted_fence() {
    let text = "- item\n  ```\n  hidden\n> ```\n> - [entry](./0001-a.md)\n> ```\nafter\n";

    assert_eq!(scanned(text), [(0, 0, "- item"), (6, 0, "after")]);
}

#[test]
fn a_line_after_a_fence_ends_is_read_as_a_fresh_block() {
    let text = "- item\n  ```\n  hidden\n> - quoted\n>   continued\nafter\n";
    let lines: Vec<(usize, usize, usize)> = unfenced_lines(text.lines())
        .map(|line| (line.index, line.item_start, line.view.depth))
        .collect();

    assert_eq!(
        scanned(text),
        [
            (0, 0, "- item"),
            (3, 0, "- quoted"),
            (4, 0, "continued"),
            (5, 0, "after"),
        ]
    );
    assert_eq!(lines, [(0, 0, 0), (3, 3, 0), (4, 3, 1), (5, 5, 0)]);
}

#[test]
fn an_over_indented_closer_does_not_close_the_fence() {
    let text = "- Topic\n    ```\n    hidden\n       ```\n    still hidden\nafter\n";

    assert_eq!(scanned(text), [(0, 0, "- Topic"), (5, 0, "after")]);
}

#[test]
fn a_quote_at_a_list_item_content_column_is_transparent() {
    let text = "10. Topic\n    > - ```\n    >   - [entry](./0001-a.md)\n    >   ```\nafter\n";

    assert_eq!(scanned(text), [(0, 0, "10. Topic"), (4, 0, "after")]);
}

#[test]
fn a_quoted_fence_hides_the_quoted_lines() {
    let cases = [
        ("plain", "> ```\n> hidden\n> ```\nafter\n"),
        (
            "list item",
            "> - ```\n>   - [entry](./0001-a.md)\n>   ```\nafter\n",
        ),
    ];

    for (case, text) in cases {
        assert_eq!(scanned(text), [(3, 0, "after")], "{case}");
    }
}

#[test]
fn a_quote_marker_is_read_up_to_three_spaces_of_indent() {
    let hidden = "   > ```\n   > hidden\n   > ```\nafter\n";
    let code = "    > hidden\nafter\n";

    assert_eq!(scanned(hidden), [(3, 0, "after")]);
    assert_eq!(scanned(code), [(1, 0, "after")]);
}

#[test]
fn a_no_break_space_does_not_open_a_list_item() {
    let text = "\u{a0}- ```\n  - [entry](./0001-a.md)\n";

    assert_eq!(
        scanned(text),
        [(0, 0, "\u{a0}- ```"), (1, 2, "- [entry](./0001-a.md)")]
    );
}

#[test]
fn a_nested_item_is_measured_from_its_own_content_column() {
    let text = "- Topic\n    - child\n";

    assert_eq!(scanned(text), [(0, 0, "- Topic"), (1, 2, "- child")]);
}

#[test]
fn a_line_names_the_item_that_holds_it() {
    let text = "- Topic\n  continued\n\n    - child\n      child continued\n- Next\n";
    let starts: Vec<usize> = unfenced_lines(text.lines())
        .map(|line| line.item_start)
        .collect();

    assert_eq!(starts, [0, 0, 0, 0, 3, 5]);
}

#[test]
fn list_item_content_drops_the_marker_and_its_spacing() {
    let cases = [
        ("dash", "- Topic", Some("Topic")),
        ("star", "* Topic", Some("Topic")),
        ("plus", "+ Topic", Some("Topic")),
        ("ordered", "1. Topic", Some("Topic")),
        ("indented", "   - Topic", Some("Topic")),
        ("three spaces after the marker", "-   Topic", Some("Topic")),
        ("no content", "- ", Some("")),
        ("not an item", "Topic", None),
        ("no space after the marker", "-Topic", None),
        ("no-break space", "\u{a0}- Topic", None),
    ];

    for (case, line, expected) in cases {
        assert_eq!(list_item_content(line), expected, "{case}");
    }
}

#[test]
fn item_content_column_counts_the_marker_its_spacing_and_tabs() {
    let cases = [
        ("dash", "- Topic", Some(2)),
        ("ordered", "12. Topic", Some(4)),
        ("indented", "  - Topic", Some(4)),
        ("tab", "\t- Topic", Some(6)),
        ("not an item", "Topic", None),
        ("no-break space", "\u{a0}- Topic", None),
    ];

    for (case, line, expected) in cases {
        assert_eq!(item_content_column(line), expected, "{case}");
    }
}

#[test]
fn opens_block_names_each_block_that_interrupts_a_paragraph() {
    let cases = [
        ("heading", "# Title", true),
        ("heading without text", "##", true),
        ("hashtag", "#tag", false),
        ("seven hashes", "####### x", false),
        ("quote", "> quote", true),
        ("html", "<div>", true),
        ("fence", "```rust", true),
        ("bullet", "- item", true),
        ("ordered", "1. item", true),
        ("dash break", "---", true),
        ("spaced star break", "* * *", true),
        ("underscore break", "___", true),
        ("two dashes", "--", false),
        ("mixed marks", "-*-", false),
        ("plain text", "More text.", false),
    ];

    for (case, line, expected) in cases {
        assert_eq!(opens_block(line), expected, "{case}");
    }
}

// Every short sequence of lines drawn from a small alphabet, so the container and fence state
// that earlier lines leave behind meets each later line.
#[test]
fn every_short_line_sequence_keeps_the_scanner_invariants() {
    let alphabet = [
        "- item", "  text", "    code", "```", "~~~", "", "# head", "1. one",
    ];
    let mut sequences: Vec<Vec<&str>> = vec![Vec::new()];

    for _ in 0..5 {
        let longer: Vec<Vec<&str>> = sequences
            .iter()
            .flat_map(|sequence| {
                alphabet.iter().map(move |line| {
                    let mut next = sequence.clone();

                    next.push(line);
                    next
                })
            })
            .collect();

        for sequence in &longer {
            let text = format!("{}\n", sequence.join("\n"));
            let lines: Vec<_> = unfenced_lines(text.lines()).collect();
            let indices: Vec<usize> = lines.iter().map(|line| line.index).collect();
            let mut sorted = indices.clone();

            sorted.sort_unstable();
            sorted.dedup();

            assert_eq!(indices, sorted, "{text:?}: indices rise");

            let hides_a_line = sequence.iter().any(|line| {
                let (indent, content) = split_indent(line);

                opening_fence(content).is_some() || indent >= INDENTED_CODE
            });

            if !hides_a_line {
                let all: Vec<usize> = (0..sequence.len()).collect();

                assert_eq!(indices, all, "{text:?}: every line is returned");
            }

            for line in &lines {
                assert!(line.item_start <= line.index, "{text:?}: item starts first");
                assert!(
                    !line.view.content.starts_with("```") && !line.view.content.starts_with("~~~"),
                    "{text:?}: a fence line is never returned"
                );
            }
        }

        sequences = longer;
    }
}
