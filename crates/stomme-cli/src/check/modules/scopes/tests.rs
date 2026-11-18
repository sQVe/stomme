use super::*;

fn strings(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|segment| segment.to_string()).collect()
}

fn file_exports(source: &str) -> Vec<(String, Target)> {
    let scopes = scopes(source);

    scopes
        .into_iter()
        .next()
        .expect("the file scope comes first")
        .exports
        .into_iter()
        .map(|export| (export.name, export.target))
        .collect()
}

#[test]
fn a_self_alias_exports_its_name_and_target() {
    assert_eq!(
        file_exports("pub use self::attention as renamed;"),
        [(
            "renamed".to_string(),
            Target::Relative {
                supers: 0,
                segments: strings(&["attention"]),
            }
        )]
    );
}

fn relative(supers: usize, segments: &[&str]) -> Target {
    Target::Relative {
        supers,
        segments: strings(segments),
    }
}

fn named(name: &str, target: Target) -> (String, Target) {
    (name.to_string(), target)
}

#[test]
fn target_forms_map_to_their_kind() {
    assert_eq!(
        file_exports(
            "pub use attention as a;\n\
             pub use super::fact::attention as b;\n\
             pub use crate::fact::attention as c;\n\
             pub use ::std::mem as d;\n"
        ),
        [
            named("a", Target::Bare(strings(&["attention"]))),
            named("b", relative(1, &["fact", "attention"])),
            named("c", Target::Crate(strings(&["fact", "attention"]))),
            named("d", Target::External),
        ]
    );
}

#[test]
fn a_group_exports_each_leaf() {
    assert_eq!(
        file_exports("pub use crate::fact::{score, attention as renamed};"),
        [
            named("score", Target::Crate(strings(&["fact", "score"]))),
            named("renamed", Target::Crate(strings(&["fact", "attention"]))),
        ]
    );
    assert_eq!(
        file_exports("pub use self::attention::{self as renamed};"),
        [named("renamed", relative(0, &["attention"]))]
    );
}

#[test]
fn an_export_without_an_alias_takes_its_last_segment() {
    assert_eq!(
        file_exports("pub use attention::helper;"),
        [named(
            "helper",
            Target::Bare(strings(&["attention", "helper"]))
        )]
    );
}

#[test]
fn underscore_and_glob_exports_bind_no_name() {
    assert_eq!(file_exports("pub use a as _;\npub use a::*;\n"), []);
}

#[test]
fn restricted_visibility_exports_and_private_use_does_not() {
    assert_eq!(
        file_exports("pub(crate) use a as b;\npub(in crate::x) use a as e;\nuse a as c;\n"),
        [
            named("b", Target::Bare(strings(&["a"]))),
            named("e", Target::Bare(strings(&["a"]))),
        ]
    );
}

#[test]
fn inline_modules_get_their_own_scope() {
    let scopes = scopes("pub mod inner { pub use super::attention as renamed; }");

    assert_eq!(scopes.len(), 2);
    assert_eq!(scopes[0].inline_path, Vec::<String>::new());
    assert_eq!(
        scopes[0].inline_modules,
        BTreeSet::from(["inner".to_string()])
    );
    assert_eq!(scopes[0].exports, []);
    assert_eq!(scopes[1].inline_path, ["inner"]);
    assert_eq!(
        scopes[1].exports,
        [Export {
            name: "renamed".to_string(),
            target: relative(1, &["attention"]),
        }]
    );
}

#[test]
fn nested_inline_modules_are_scopes_in_source_order() {
    let scopes = scopes(
        "mod a { mod b { pub use x as one; } pub use x as two; }\n\
         mod c { pub use x as three; }\n\
         pub use x as four;\n",
    );
    let summary: Vec<(Vec<String>, Vec<String>, Vec<String>)> = scopes
        .iter()
        .map(|scope| {
            (
                scope.inline_path.clone(),
                scope.inline_modules.iter().cloned().collect(),
                scope
                    .exports
                    .iter()
                    .map(|export| export.name.clone())
                    .collect(),
            )
        })
        .collect();

    assert_eq!(
        summary,
        [
            (strings(&[]), strings(&["a", "c"]), strings(&["four"])),
            (strings(&["a"]), strings(&["b"]), strings(&["two"])),
            (strings(&["a", "b"]), strings(&[]), strings(&["one"])),
            (strings(&["c"]), strings(&[]), strings(&["three"])),
        ]
    );
}

#[test]
fn exports_inside_functions_impls_attributes_and_macros_are_skipped() {
    let scopes = scopes(
        "fn f() { pub use a as b; mod hidden { pub use a as h; } }\n\
         discard!(pub use a as c;);\n\
         discard![pub use a as d;];\n\
         impl X { }\n",
    );

    assert_eq!(scopes.len(), 1);
    assert_eq!(scopes[0].exports, []);
    assert_eq!(scopes[0].inline_modules, BTreeSet::new());
}

#[test]
fn a_stray_close_delimiter_does_not_panic() {
    assert_eq!(
        file_exports("} ) ]\npub use a as b;\n"),
        [named("b", Target::Bare(strings(&["a"])))]
    );
}

#[test]
fn cfg_attributes_do_not_hide_exports() {
    assert_eq!(
        file_exports("#[cfg(not(test))] pub use self::attention as renamed;"),
        [named("renamed", relative(0, &["attention"]))]
    );
}

#[test]
fn a_leading_colon_in_one_group_leaf_marks_only_that_leaf() {
    assert_eq!(
        file_exports("pub use {::std::mem as a, self::b as c};"),
        [
            named("a", Target::External),
            named("c", relative(0, &["b"]))
        ]
    );
}
