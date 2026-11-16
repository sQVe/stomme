use super::*;

fn segments(source: &str) -> Vec<Vec<String>> {
    paths(source)
        .into_iter()
        .map(|path| path.segments)
        .collect()
}

fn lines(source: &str) -> Vec<usize> {
    paths(source).into_iter().map(|path| path.line).collect()
}

fn expected(paths: &[&[&str]]) -> Vec<Vec<String>> {
    paths
        .iter()
        .map(|path| path.iter().map(|segment| segment.to_string()).collect())
        .collect()
}

#[test]
fn nested_group_yields_one_full_path_per_leaf() {
    assert_eq!(
        segments("use crate::{ticket::{Ticket, store}, github};"),
        expected(&[
            &["crate", "ticket", "Ticket"],
            &["crate", "ticket", "store"],
            &["crate", "github"],
        ])
    );
}

#[test]
fn identifier_after_a_group_closes_is_not_yielded() {
    assert_eq!(
        segments("use crate::{ticket}; fn store() {}"),
        expected(&[&["crate", "ticket"]])
    );
}

#[test]
fn later_path_segment_in_a_group_is_yielded() {
    assert_eq!(
        segments("use crate::{ticket::store};"),
        expected(&[&["crate", "ticket", "store"]])
    );
}

#[test]
fn raw_identifier_yields_its_name_without_the_prefix() {
    assert_eq!(
        segments("use crate::r#store::read;\nuse crate::{r#sync, ticket};\n"),
        expected(&[
            &["crate", "store", "read"],
            &["crate", "sync"],
            &["crate", "ticket"],
        ])
    );
}

#[test]
fn glob_use_yields_the_glob_and_nothing_after_it() {
    assert_eq!(
        segments("use crate::*;\ntype Pair = (u8, store::Id);\nuse crate::{*, ticket};\n"),
        expected(&[&["crate", "*"], &["crate", "*"], &["crate", "ticket"]])
    );
}

#[test]
fn line_after_a_shebang_counts_the_shebang_line() {
    assert_eq!(
        lines("#!/usr/bin/env stomme\n\nuse crate::store::read;\n"),
        [3]
    );
}

#[test]
fn super_path_keeps_every_super_segment() {
    assert_eq!(
        segments("use super::super::store::read;"),
        expected(&[&["super", "super", "store", "read"]])
    );
}

#[test]
fn inline_module_is_recorded_for_its_paths() {
    let paths = paths("mod tests {\n    use super::*;\n}\n");

    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].segments, expected(&[&["super", "*"]])[0]);
    assert_eq!(paths[0].inline_modules, ["tests"]);
}

#[test]
fn nested_inline_modules_are_recorded_in_order() {
    let paths = paths("mod outer {\n    mod inner {\n        use super::store;\n    }\n}\n");

    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].inline_modules, ["outer", "inner"]);
}

#[test]
fn a_module_declaration_without_a_body_is_not_an_inline_module() {
    let paths = paths("mod tests;\nuse super::store;\n");

    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].inline_modules, Vec::<String>::new());
}

#[test]
fn group_parsing_stops_at_early_or_unexpected_tokens() {
    let cases: [(&str, &[&[&str]]); 2] = [
        ("use crate::{a, ;", &[&["crate", "a"], &["crate"]]),
        (
            "use crate::{a as b, #c}",
            &[
                &["crate", "a"],
                &["crate", "b"],
                &["crate"],
                &["crate", "c"],
            ],
        ),
    ];

    for (source, expected_paths) in cases {
        assert_eq!(segments(source), expected(expected_paths), "{source}");
    }
}

#[test]
fn uses_in_comments_and_strings_are_ignored() {
    assert_eq!(
        segments(
            "// use crate::store::read;\n/* crate::store */\nconst A: &str = \"crate::store\";\n"
        ),
        Vec::<Vec<String>>::new()
    );
}
