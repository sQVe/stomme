use super::*;

fn scopes(source: &str) -> Vec<Scope> {
    super::scopes(source, Edition::Other, false)
}

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
             pub use ::std::mem as d;\n\
             pub use super::super::a as e;\n"
        ),
        [
            named("a", Target::Bare(strings(&["attention"]))),
            named("b", relative(1, &["fact", "attention"])),
            named("c", Target::Crate(strings(&["fact", "attention"]))),
            named("d", Target::External),
            named("e", relative(2, &["a"])),
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
fn a_self_leaf_without_a_prefix_exports_nothing() {
    assert_eq!(file_exports("pub use {self as x};\npub use {self};\n"), []);
}

#[test]
fn a_leading_colon_before_a_prefixed_group_marks_its_leaves() {
    assert_eq!(
        file_exports("pub use ::std::{mem as r};"),
        [named("r", Target::External)]
    );
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
            visibility: ExportVisibility::Public,
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
         #[doc(pub use a as e;)]\n\
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

#[test]
fn an_identifier_other_than_pub_before_use_exports_nothing() {
    assert_eq!(
        file_exports("crate use self::attention as renamed;"),
        Vec::new()
    );
}

fn file_scope(source: &str) -> Scope {
    scopes(source)
        .into_iter()
        .next()
        .expect("the file scope comes first")
}

fn names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn scope_at(scopes: &[Scope], inline_path: &[&str]) -> usize {
    scopes
        .iter()
        .position(|scope| scope.inline_path == strings(inline_path))
        .expect("the scope exists")
}

#[test]
fn public_module_declarations_are_listed_per_scope() {
    let source = "pub mod a;\nmod b;\npub(crate) mod c;\npub mod d {\n    pub mod e;\n    mod f;\n}\nmod g {}\npub(crate) mod h {\n    pub mod i;\n}\n";
    let scopes = scopes(source);

    assert_eq!(scopes[0].public_modules, names(&["a", "c", "d", "h"]));
    assert_eq!(scopes[0].inline_modules, names(&["d", "g", "h"]));

    let d = &scopes[scope_at(&scopes, &["d"])];
    assert_eq!(d.public_modules, names(&["e"]));
    assert_eq!(d.inline_modules, names(&[]));

    let h = &scopes[scope_at(&scopes, &["h"])];
    assert_eq!(h.public_modules, names(&["i"]));
}

#[test]
fn glob_exports_are_listed_as_targets() {
    let source = "pub use attention::*;\npub use self::*;\npub use super::*;\npub use crate::fact::*;\npub use ::std::*;\npub use self::{score, attention::*};\nuse private::*;\n";
    let scope = file_scope(source);

    assert_eq!(
        scope.globs,
        [
            Target::Bare(strings(&["attention"])),
            relative(0, &[]),
            relative(1, &[]),
            Target::Crate(strings(&["fact"])),
            Target::External,
            relative(0, &["attention"]),
        ]
    );
    assert_eq!(
        file_exports(source),
        [named("score", relative(0, &["score"]))]
    );
}

#[test]
fn crate_and_super_aliases_export_their_ancestor_targets() {
    assert_eq!(
        file_exports("pub(crate) use crate as renamed;\npub use super as up;\n"),
        [
            named("renamed", Target::Crate(Vec::new())),
            named("up", relative(1, &[])),
        ]
    );
}

#[test]
fn globs_and_public_modules_inside_functions_are_skipped() {
    let scopes = scopes("fn f() { pub use a::*; pub mod x; }\n");

    for scope in &scopes {
        assert_eq!(scope.globs, []);
        assert_eq!(scope.public_modules, names(&[]));
    }
}

#[test]
fn value_items_are_recorded_at_item_level() {
    let scope = file_scope(
        "fn a() {}\n\
         const B: u8 = 0;\n\
         static mut C: u8 = 0;\n\
         static D: u8 = 0;\n\
         const fn d() {}\n\
         struct E;\n\
         trait T { fn f(); }\n\
         impl E { fn g() {} }\n\
         fn h() { fn i() {} static J: u8 = 0; }\n",
    );

    assert_eq!(scope.value_items, names(&["a", "B", "C", "D", "d", "h"]));
}

#[test]
fn an_export_records_its_visibility() {
    let exports: Vec<(String, ExportVisibility)> = file_scope(
        "pub use a as public;\n\
         pub(self) use a as private;\n\
         pub(in self) use a as in_self;\n\
         pub(crate) use a as crate_wide;\n\
         pub(in crate::x) use a as in_crate;\n\
         use a as plain;\n",
    )
    .exports
    .into_iter()
    .map(|export| (export.name, export.visibility))
    .collect();

    assert_eq!(
        exports,
        [
            ("public".to_string(), ExportVisibility::Public),
            ("private".to_string(), ExportVisibility::Restricted),
            ("in_self".to_string(), ExportVisibility::Restricted),
            ("crate_wide".to_string(), ExportVisibility::Crate),
            ("in_crate".to_string(), ExportVisibility::Restricted),
        ]
    );
}

#[test]
fn pub_super_is_crate_visible_only_at_a_crate_root_module_file_scope() {
    let at_crate_root = super::scopes("pub(super) use a as b;", Edition::Other, true);
    let nested_file = super::scopes("pub(super) use a as b;", Edition::Other, false);
    let inline_module = super::scopes("mod inner { pub(super) use a as b; }", Edition::Other, true);

    assert_eq!(
        at_crate_root[0].exports[0].visibility,
        ExportVisibility::Crate
    );
    assert_eq!(
        nested_file[0].exports[0].visibility,
        ExportVisibility::Restricted
    );
    assert_eq!(
        inline_module[1].exports[0].visibility,
        ExportVisibility::Restricted
    );
}

#[test]
fn const_generics_and_modifiers_are_not_value_items() {
    let scope = file_scope(
        "struct S<const N: usize>;\n\
         fn f<const M: usize>() {}\n\
         const unsafe fn g() {}\n\
         const extern \"C\" fn h() {}\n\
         const I: u8 = 0;\n",
    );

    assert_eq!(scope.value_items, names(&["f", "g", "h", "I"]));
}

#[test]
fn a_leading_colon_target_is_a_crate_target_in_edition_2015() {
    let mut scopes = super::scopes("pub use ::std::mem as a;", Edition::Rust2015, false);
    let exports = scopes.remove(0).exports;

    assert_eq!(
        exports,
        [Export {
            name: "a".to_string(),
            target: Target::Crate(strings(&["std", "mem"])),
            visibility: ExportVisibility::Public,
        }]
    );
}
