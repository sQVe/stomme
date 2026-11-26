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
fn a_nested_item_is_measured_from_its_own_content_column() {
    let text = "- Topic\n    - child\n";

    assert_eq!(scanned(text), [(0, 0, "- Topic"), (1, 2, "- child")]);
}

#[test]
fn a_line_names_the_item_that_holds_it() {
    let text = "- Topic\n  continued\n\n    - child\n- Next\n";
    let starts: Vec<usize> = unfenced_lines(text.lines())
        .map(|line| line.item_start)
        .collect();

    assert_eq!(starts, [0, 0, 0, 0, 4]);
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
    ];

    for (case, line, expected) in cases {
        assert_eq!(list_item_content(line), expected, "{case}");
    }
}
