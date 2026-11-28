use super::super::scopes::{self, Scope};
use super::*;

const NESTED_FILE_DEPTH: usize = 2;

fn paths(source: &str) -> Vec<UsePath> {
    paths_in(source, Edition::Other)
}

fn paths_in(source: &str, edition: Edition) -> Vec<UsePath> {
    // Bindings ignore visibility, so the depth only has to name a file below the crate root.
    let table = binding_table(scopes::scopes(source, edition, NESTED_FILE_DEPTH));

    super::paths(source, edition, &table)
}

fn binding_table(scopes: Vec<Scope>) -> BindingTable {
    let mut table = BindingTable::new();

    for scope in scopes {
        let bindings = table.entry(scope.inline_path).or_default();

        for binding in scope.bindings {
            bindings.insert(binding.name.clone(), binding);
        }
    }

    table
}

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

#[test]
fn a_use_alias_prefixes_a_later_value_path() {
    let paths = paths("use crate::config as cfg;\ncfg::parse::read();\n");

    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].segments, expected(&[&["crate", "config"]])[0]);
    assert_eq!(paths[0].namespace, Namespace::Type);
    assert_eq!(
        paths[1].segments,
        expected(&[&["crate", "config", "parse", "read"]])[0]
    );
    assert_eq!(paths[1].namespace, Namespace::Value);
    assert_eq!(paths[1].line, 2);
}

#[test]
fn crate_and_parent_aliases_prefix_later_paths() {
    let paths = paths(
        "use crate as root;\nroot::store::read();\nuse super as parent;\nparent::other::read();\n",
    );
    let segments: Vec<Vec<String>> = paths.into_iter().map(|path| path.segments).collect();

    assert_eq!(
        segments,
        expected(&[&["crate", "store", "read"], &["super", "other", "read"]])
    );
}

#[test]
fn a_use_alias_binds_only_inside_its_inline_module() {
    let paths = paths("mod a {\n    use crate::config as cfg;\n}\ncfg::parse();\n");
    let segments: Vec<Vec<String>> = paths.into_iter().map(|path| path.segments).collect();

    assert_eq!(segments, expected(&[&["crate", "config"]]));
}

#[test]
fn a_glob_is_not_bound_as_a_prefix() {
    let paths = paths("use crate::config::*;\nconfig::parse();\n");
    let segments: Vec<Vec<String>> = paths.into_iter().map(|path| path.segments).collect();

    assert_eq!(segments, expected(&[&["crate", "config", "*"]]));
}

#[test]
fn a_value_path_is_a_value_namespace_path() {
    let paths = paths("crate::store::read();\n");

    assert_eq!(paths[0].namespace, Namespace::Value);
}

#[test]
fn edition_2015_emits_a_leading_colon_path_inside_the_crate() {
    let paths = paths_in("use ::config::parse::read;\n", Edition::Rust2015);

    assert_eq!(paths.len(), 1);
    assert_eq!(
        paths[0].segments,
        expected(&[&["crate", "config", "parse", "read"]])[0]
    );
}

#[test]
fn a_leading_colon_path_is_skipped_without_edition_2015() {
    assert!(paths_in("use ::config::parse::read;\n", Edition::Other).is_empty());
}

#[test]
fn a_multiline_value_path_reports_the_line_of_its_first_segment_after_the_root() {
    assert_eq!(lines("crate::\n    store::read();\n"), [2]);
}

#[test]
fn edition_2015_binds_a_leading_colon_alias() {
    let paths = paths_in(
        "use ::config as cfg;\ncfg::parse::read();\n",
        Edition::Rust2015,
    );
    let segments: Vec<Vec<String>> = paths.into_iter().map(|path| path.segments).collect();

    assert_eq!(
        segments,
        expected(&[&["crate", "config"], &["crate", "config", "parse", "read"],])
    );
}

#[test]
fn a_type_annotation_before_a_leading_colon_is_not_a_path_start() {
    assert!(segments("use super as parent;\nfn f(parent: ::std::sync::Arc<u8>) {}\n").is_empty());
}

#[test]
fn edition_2015_reads_a_leading_colon_after_a_generic_close() {
    let paths = paths_in(
        "fn f() {\n    x > ::config::LIMIT;\n    Foo::<{ N }>::bar();\n}\n",
        Edition::Rust2015,
    );
    let segments: Vec<Vec<String>> = paths.into_iter().map(|path| path.segments).collect();

    assert_eq!(segments, expected(&[&["crate", "config", "LIMIT"]]));
}

#[test]
fn edition_2015_ignores_an_earlier_comparison_when_reading_a_leading_colon() {
    let paths = paths_in(
        "fn smaller(a: u8, b: u8) -> bool {\n    a < b\n}\n\nfn compare() {\n    if 2 > ::config::LIMIT {\n        println!(\"big\");\n    }\n}\n",
        Edition::Rust2015,
    );
    let segments: Vec<Vec<String>> = paths.into_iter().map(|path| path.segments).collect();

    assert_eq!(segments, expected(&[&["crate", "config", "LIMIT"]]));
}

#[test]
fn edition_2015_ignores_a_same_block_comparison_when_reading_a_leading_colon() {
    let paths = paths_in(
        "fn compare(a: u8, b: u8, c: u8) {\n    if a < b && c > ::config::LIMIT {\n        println!(\"big\");\n    }\n}\n",
        Edition::Rust2015,
    );
    let segments: Vec<Vec<String>> = paths.into_iter().map(|path| path.segments).collect();

    assert_eq!(segments, expected(&[&["crate", "config", "LIMIT"]]));
}

#[test]
fn edition_2015_reads_no_leading_colon_after_a_nested_generic() {
    let paths = paths_in(
        "fn f() {\n    Foo<Bar<u8>>::baz();\n    Foo::<{ N }>::bar();\n}\n",
        Edition::Rust2015,
    );

    assert!(paths.is_empty());
}

#[test]
fn edition_2015_marks_an_external_absolute_alias_as_an_implicit_crate_root() {
    let paths = paths_in(
        "use ::std as std_alias;\nstd_alias::mem();\n",
        Edition::Rust2015,
    );

    assert_eq!(paths[1].segments, expected(&[&["crate", "std", "mem"]])[0]);
    assert!(paths[1].implicit_crate_root);
}

#[test]
fn a_leading_colon_after_a_cast_is_a_path_in_edition_2015() {
    let paths = paths_in(
        "fn f() {\n    let value = 0 as ::config::Ty;\n}\n",
        Edition::Rust2015,
    );

    assert_eq!(paths.len(), 1);
    assert_eq!(
        paths[0].segments,
        expected(&[&["crate", "config", "Ty"]])[0]
    );
}
