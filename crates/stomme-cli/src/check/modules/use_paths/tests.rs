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
fn path_after_an_inline_module_closes_keeps_no_inline_module() {
    let paths = paths("mod a {\n    use super::store;\n}\nuse super::store;\n");

    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].inline_modules, ["a"]);
    assert_eq!(paths[1].inline_modules, Vec::<String>::new());
}

#[test]
fn path_after_a_nested_inline_module_closes_stays_in_the_outer_module() {
    let paths =
        paths("mod a {\n    mod b {\n        use super::super::x;\n    }\n    use super::y;\n}\n");

    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].segments, expected(&[&["super", "super", "x"]])[0]);
    assert_eq!(paths[0].inline_modules, ["a", "b"]);
    assert_eq!(paths[1].segments, expected(&[&["super", "y"]])[0]);
    assert_eq!(paths[1].inline_modules, ["a"]);
}

#[test]
fn an_identifier_before_a_brace_is_not_an_inline_module() {
    let paths = paths("impl Store {\n    use super::store;\n}\n");

    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].inline_modules, Vec::<String>::new());
}

#[test]
fn a_single_colon_after_crate_is_not_a_path_start() {
    assert_eq!(
        segments("crate: value\nuse crate::store;\n"),
        expected(&[&["crate", "store"]])
    );
}

#[test]
fn a_single_colon_after_a_path_segment_ends_the_path() {
    assert_eq!(
        segments("use crate::store: value other;\nuse crate::other;\n"),
        expected(&[&["crate", "store"], &["crate", "other"]])
    );
}

#[test]
fn a_module_declaration_without_a_body_is_not_an_inline_module() {
    let paths = paths("mod tests;\nuse super::store;\n");

    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].inline_modules, Vec::<String>::new());
}

#[test]
fn alias_in_a_group_is_not_a_second_path() {
    assert_eq!(
        segments("use crate::{store as alias};"),
        expected(&[&["crate", "store"]])
    );
}

#[test]
fn self_in_a_group_resolves_to_the_group_prefix() {
    assert_eq!(
        segments("use crate::store::{self, Read};"),
        expected(&[&["crate", "store"], &["crate", "store", "Read"]])
    );
}

#[test]
fn self_alias_of_the_crate_root_yields_the_crate_path() {
    assert_eq!(
        segments("use crate::{self as root, store};"),
        expected(&[&["crate"], &["crate", "store"]])
    );
}

#[test]
fn a_stray_close_brace_does_not_panic() {
    assert_eq!(
        segments("}\nuse crate::store;\n"),
        expected(&[&["crate", "store"]])
    );
}

#[test]
fn group_parsing_stops_at_early_or_unexpected_tokens() {
    let cases: [(&str, &[&[&str]]); 2] = [
        ("use crate::{a, ;", &[&["crate", "a"], &["crate"]]),
        (
            "use crate::{a as b, #c}",
            &[&["crate", "a"], &["crate"], &["crate", "c"]],
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

#[test]
fn in_test_module_follows_cfg_test_attributes_and_nesting() {
    let paths = paths(
        "#[cfg(test)]\nmod a {\n    mod b {\n        use crate::x;\n    }\n}\nuse crate::y;\n",
    );

    assert_eq!(paths.len(), 2);
    assert!(paths[0].in_test_module);
    assert!(!paths[1].in_test_module);
}
