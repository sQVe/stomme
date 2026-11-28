use super::Resolver;
use crate::check::Problem;
use crate::check::modules::tests::{assert_problem, problems};
use crate::check::modules::{read_modules, read_sources};
use crate::check::source_files;
use crate::check::test_support::project;
use crate::config::{self, Installation};
use crate::layer::{self, Language};
use crate::rule::Rule;

const ATTENTION: &str = "ticket:src/fact/attention.rs";
const ALIAS_ENTRY: &str = "pub mod attention;\npub use self::attention as renamed;\n";
const RENAMED_HELPER: &str = "use crate::fact::renamed::helper;\n";
const RENAMED: &str = "use crate::fact::renamed;\n";
const ALTERNATE_HELPER: &str = "use crate::fact::alternate::helper;\n";
const FUNCTION_AND_MODULE: &str = "pub mod attention;\npub use self::attention as renamed;\npub use self::attention::helper as renamed;\n";
const FUNCTION_THEN_MODULE: &str = "pub mod attention;\npub use self::attention::helper as renamed;\npub use self::attention as renamed;\n";

struct Case {
    name: &'static str,
    fact_entry: &'static str,
    extra_files: &'static [(&'static str, &'static str)],
    extra_rows: &'static str,
    ticket: &'static str,
    keys: &'static [&'static str],
}

fn case(
    name: &'static str,
    fact_entry: &'static str,
    ticket: &'static str,
    keys: &'static [&'static str],
) -> Case {
    Case {
        name,
        fact_entry,
        extra_files: &[],
        extra_rows: "",
        ticket,
        keys,
    }
}

#[test]
fn module_alias_use_reports_the_private_file_it_reaches() {
    let root = alias_project(ALIAS_ENTRY, &[], "", RENAMED_HELPER);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModulePrivateFile,
        ATTENTION,
        "module alias",
    );
}

#[test]
fn module_alias_cases_decide_the_reached_private_file() {
    let cases = [
        case(
            "bare target",
            "pub mod attention;\npub use attention as renamed;\n",
            RENAMED_HELPER,
            &[ATTENTION],
        ),
        case(
            "super target",
            "pub mod attention;\npub use super::fact::attention as renamed;\n",
            RENAMED_HELPER,
            &[ATTENTION],
        ),
        case(
            "crate target",
            "pub mod attention;\npub use crate::fact::attention as renamed;\n",
            RENAMED_HELPER,
            &[ATTENTION],
        ),
        case(
            "grouped self target",
            "pub mod attention;\npub use self::{attention as renamed};\n",
            RENAMED_HELPER,
            &[ATTENTION],
        ),
        case(
            "grouped crate target with a sibling",
            "pub mod attention;\npub mod score;\npub use crate::fact::{score, attention as renamed};\n",
            RENAMED_HELPER,
            &[ATTENTION],
        ),
        case(
            "self inside a group",
            "pub mod attention;\npub use self::attention::{self as renamed};\n",
            RENAMED_HELPER,
            &[ATTENTION],
        ),
        case(
            "terminal path to a module alias",
            ALIAS_ENTRY,
            RENAMED,
            &[ATTENTION],
        ),
        case(
            "chain",
            "pub mod attention;\npub use self::attention as renamed;\npub use self::renamed as alternate;\n",
            ALTERNATE_HELPER,
            &[ATTENTION],
        ),
        case(
            "chain declared before its target",
            "pub mod attention;\npub use self::renamed as alternate;\npub use self::attention as renamed;\n",
            ALTERNATE_HELPER,
            &[ATTENTION],
        ),
        case(
            "self alias of the module",
            "pub mod attention;\npub use crate::fact as renamed;\n",
            "use crate::fact::renamed::renamed::attention::helper;\n",
            &[ATTENTION],
        ),
        case(
            "module then function, terminal",
            FUNCTION_AND_MODULE,
            RENAMED,
            &[],
        ),
        case(
            "function then module, terminal",
            FUNCTION_THEN_MODULE,
            RENAMED,
            &[],
        ),
        case(
            "module then function, deeper path",
            FUNCTION_AND_MODULE,
            RENAMED_HELPER,
            &[ATTENTION],
        ),
        case(
            "function then module, deeper path",
            FUNCTION_THEN_MODULE,
            RENAMED_HELPER,
            &[ATTENTION],
        ),
        case(
            "both meanings through a chain, terminal",
            "pub mod attention;\npub use self::attention as renamed;\npub use self::attention::helper as renamed;\npub use self::renamed as alternate;\n",
            "use crate::fact::alternate;\n",
            &[],
        ),
        case(
            "both meanings through a chain, deeper path",
            "pub mod attention;\npub use self::attention as renamed;\npub use self::attention::helper as renamed;\npub use self::renamed as alternate;\n",
            ALTERNATE_HELPER,
            &[ATTENTION],
        ),
        Case {
            extra_files: &[("src/fact/inner/deep.rs", "pub fn helper() {}\n")],
            ..case(
                "alias to an inline module, then a private file",
                "mod inner {\n    pub mod deep;\n}\npub use self::inner as renamed;\n",
                "use crate::fact::renamed::deep::helper;\n",
                &["ticket:src/fact/inner/deep.rs"],
            )
        },
        case(
            "alias declared inside an inline module",
            "pub mod attention;\npub mod inner {\n    pub use super::attention as renamed;\n}\n",
            "use crate::fact::inner::renamed::helper;\n",
            &[ATTENTION],
        ),
        case(
            "item re-exported through an alias",
            "pub mod attention;\npub use self::attention as renamed;\npub use self::renamed::helper as public_helper;\n",
            "use crate::fact::public_helper;\n",
            &[],
        ),
        case(
            "leading colon target",
            "pub mod attention;\npub use ::std::mem as renamed;\n",
            "use crate::fact::renamed::drop;\n",
            &[],
        ),
        case(
            "bare external target",
            "pub mod attention;\npub use serde::de as renamed;\n",
            RENAMED_HELPER,
            &[],
        ),
        case(
            "grouped leading colon target",
            "pub mod attention;\npub use ::std::{mem as renamed};\n",
            "use crate::fact::renamed::drop;\n",
            &[],
        ),
        case(
            "export inside a macro call",
            "pub mod attention;\ndiscard!(pub use ::std::mem::drop as attention;);\n",
            "use crate::fact::attention;\n",
            &[ATTENTION],
        ),
        Case {
            extra_rows: "\n[modules.attention]\nentry = \"src/fact/attention.rs\"\n",
            ..case(
                "alias into a deeper row",
                "pub mod attention;\npub use attention as renamed;\n",
                RENAMED_HELPER,
                &[],
            )
        },
    ];

    for case in cases {
        let root = alias_project(
            case.fact_entry,
            case.extra_files,
            case.extra_rows,
            case.ticket,
        );
        let problems = problems(root.path());

        assert_keys(&problems, case.keys, case.name);
    }
}

const PRIVATE_INNER: &str = "mod attention;\npub use attention::inner as renamed;\n";
const INLINE_INNER: &str = "pub mod inner { pub fn helper() {} }\n";
const NESTED_INLINE: &str = "pub mod outer { pub mod inner { pub fn helper() {} } }\n";

#[test]
fn module_alias_into_a_private_file_inline_module_reports_the_file() {
    let root = alias_project(
        PRIVATE_INNER,
        &[("src/fact/attention.rs", INLINE_INNER)],
        "",
        RENAMED_HELPER,
    );

    let problems = problems(root.path());

    assert_keys(&problems, &[ATTENTION], "alias to an inline module");
}

#[test]
fn module_aliases_into_private_file_inline_modules_decide_the_reached_file() {
    let cases = [
        Case {
            extra_files: &[("src/fact/attention.rs", INLINE_INNER)],
            ..case(
                "N1 inline module in a private file",
                PRIVATE_INNER,
                RENAMED_HELPER,
                &[ATTENTION],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", NESTED_INLINE)],
            ..case(
                "N2 two-level inline nesting",
                "mod attention;\npub use attention::outer::inner as renamed;\n",
                RENAMED_HELPER,
                &[ATTENTION],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", NESTED_INLINE)],
            ..case(
                "N3 alias to the outer inline module",
                "mod attention;\npub use attention::outer as renamed;\n",
                "use crate::fact::renamed::inner::helper;\n",
                &[ATTENTION],
            )
        },
        Case {
            extra_files: &[
                ("src/fact/attention.rs", "pub mod inner { pub mod deep; }\n"),
                ("src/fact/attention/inner/deep.rs", "pub fn helper() {}\n"),
            ],
            ..case(
                "N4 file module inside an inline module",
                PRIVATE_INNER,
                "use crate::fact::renamed::deep::helper;\n",
                &["ticket:src/fact/attention/inner/deep.rs"],
            )
        },
        Case {
            extra_files: &[(
                "src/fact/attention.rs",
                "pub mod inner { pub use super::super::score as deeper; }\n",
            )],
            ..case(
                "N5 pub use inside the inline module is not followed",
                "mod attention;\nmod score;\npub use attention::inner as renamed;\n",
                "use crate::fact::renamed::deeper::rank;\n",
                &[ATTENTION],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", "pub fn inner() {}\n")],
            ..case(
                "N6 item with the inline name, terminal",
                PRIVATE_INNER,
                RENAMED,
                &[],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", INLINE_INNER)],
            ..case(
                "N7 inline module, terminal use of the alias",
                PRIVATE_INNER,
                RENAMED,
                &[ATTENTION],
            )
        },
    ];

    assert_cases(cases);
}

const MODE_ENTRY: &str = "mod attention;\npub use attention::Mode;\n";
const MODE_FILE: &str = "pub enum Mode { A, B }\n";
const CONFIG_ENTRY: &str = "mod attention;\npub use attention::Config;\n";
const CONFIG_FILE: &str =
    "pub struct Config;\nimpl Config { pub fn default() -> Self { Config } }\n";
const INNER_MODE_ENTRY: &str = "mod attention;\npub use attention::inner::Mode;\n";
const INNER_MODE_FILE: &str = "pub mod inner { pub enum Mode { A, B } }\n";
const NESTED_ENTRY: &str = "mod attention;\npub use attention::outer::inner as renamed;\n";

#[test]
fn a_re_exported_enum_variant_path_is_not_a_private_file_use() {
    let root = alias_project(
        MODE_ENTRY,
        &[("src/fact/attention.rs", MODE_FILE)],
        "",
        "use crate::fact::Mode::A;\n",
    );

    let problems = problems(root.path());

    assert_keys(&problems, &[], "variant path through a re-exported enum");
}

#[test]
fn re_exported_items_and_inline_modules_of_private_files_decide_the_reached_file() {
    let cases = [
        Case {
            extra_files: &[("src/fact/attention.rs", MODE_FILE)],
            ..case(
                "V1 re-exported enum, variant path",
                MODE_ENTRY,
                "use crate::fact::Mode::A;\n",
                &[],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", MODE_FILE)],
            ..case(
                "V2 re-exported enum, renamed",
                MODE_ENTRY,
                "use crate::fact::Mode as M;\n",
                &[],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", MODE_FILE)],
            ..case(
                "V3 re-exported enum, group",
                MODE_ENTRY,
                "use crate::fact::Mode::{A, B};\n",
                &[],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", CONFIG_FILE)],
            ..case(
                "V4 re-exported struct, associated item",
                CONFIG_ENTRY,
                "use crate::fact::Config::default;\n",
                &[],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", INNER_MODE_FILE)],
            ..case(
                "V5 enum inside an inline module",
                INNER_MODE_ENTRY,
                "use crate::fact::Mode::A;\n",
                &[],
            )
        },
        Case {
            extra_files: &[("src/fact/attention.rs", NESTED_INLINE)],
            ..case(
                "V6 two-level inline module, terminal use",
                NESTED_ENTRY,
                RENAMED,
                &[ATTENTION],
            )
        },
    ];

    assert_cases(cases);
}

#[test]
fn reexport_from_a_private_file_keeps_the_item_file_as_the_result() {
    let files = [
        (
            "src/config/models.rs",
            "pub mod internal;\npub use internal::X;\n",
        ),
        ("src/config/models/internal.rs", "pub struct X;\n"),
        ("src/config/models/X.rs", "pub struct X;\n"),
    ];
    let root = nested_reexport_project(&[], &files, "use crate::config::models::X;\n");

    let problems = problems(root.path());

    assert_reexport(
        &problems,
        Some("ticket:src/config/models/X.rs"),
        "N8 private-file result kept",
    );
}

#[test]
fn alias_cycles_terminate_without_a_problem() {
    let cases = [
        (
            "two-name cycle",
            "pub mod attention;\npub use self::first as second;\npub use self::second as first;\n",
            "use crate::fact::first::helper;\n",
        ),
        (
            "self-cycle",
            "pub mod attention;\npub use self::looped;\n",
            "use crate::fact::looped::helper;\n",
        ),
    ];

    for (name, fact_entry, ticket) in cases {
        let root = alias_project(fact_entry, &[], "", ticket);
        let problems = problems(root.path());

        assert_keys(&problems, &[], name);
    }
}

#[test]
fn cfg_selected_module_and_alias_of_one_name_report_both_files() {
    let fact_entry = "pub mod attention;\n#[cfg(feature = \"split\")]\npub mod renamed;\n#[cfg(not(feature = \"split\"))]\npub use self::attention as renamed;\n";
    let root = alias_project(
        fact_entry,
        &[("src/fact/renamed.rs", "pub fn helper() {}\n")],
        "",
        RENAMED,
    );

    let problems = problems(root.path());

    assert_keys(
        &problems,
        &[ATTENTION, "ticket:src/fact/renamed.rs"],
        "cfg selected module and alias",
    );
}

const CFG_INNER_ALIAS: &str = "use crate::fact::inner::renamed::helper;\n";

const CFG_ALIAS_FIRST: &str = "pub mod attention;\n#[cfg(a)]\npub mod inner { pub use super::attention as renamed; }\n#[cfg(not(a))]\npub mod inner { pub fn other() {} }\n";

#[test]
fn cfg_selected_inline_modules_of_one_name_merge_their_exports() {
    let root = alias_project(CFG_ALIAS_FIRST, &[], "", CFG_INNER_ALIAS);

    let problems = problems(root.path());

    assert_keys(&problems, &[ATTENTION], "B1 alias in the first branch");
}

#[test]
fn cfg_selected_inline_modules_of_one_name_decide_the_reached_private_file() {
    assert_cases([
        case(
            "B1 alias in the first branch",
            CFG_ALIAS_FIRST,
            CFG_INNER_ALIAS,
            &[ATTENTION],
        ),
        case(
            "B2 alias in the last branch",
            "pub mod attention;\n#[cfg(not(a))]\npub mod inner { pub fn other() {} }\n#[cfg(a)]\npub mod inner { pub use super::attention as renamed; }\n",
            CFG_INNER_ALIAS,
            &[ATTENTION],
        ),
        case(
            "B3 nested inline module in the first branch",
            "pub mod attention;\n#[cfg(a)]\npub mod inner { pub mod nested { pub use super::super::attention as renamed; } }\n#[cfg(not(a))]\npub mod inner {}\n",
            "use crate::fact::inner::nested::renamed::helper;\n",
            &[ATTENTION],
        ),
        case(
            "B4 different aliases of one name",
            "pub mod attention;\npub mod score;\n#[cfg(a)]\npub mod inner { pub use super::attention as renamed; }\n#[cfg(not(a))]\npub mod inner { pub use super::score as renamed; }\n",
            CFG_INNER_ALIAS,
            &[ATTENTION, "ticket:src/fact/score.rs"],
        ),
    ]);
}

#[test]
fn item_with_its_file_name_reexported_by_the_entry_is_not_a_private_file() {
    let root = alias_project(
        "mod attention;\npub use attention::attention;\n",
        &[],
        "",
        "use crate::fact::attention;\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn entry_reexports_decide_whether_the_child_file_is_private() {
    let cases = [
        (
            "reexported function",
            "mod attention;\npub use attention::helper;\n",
            "use crate::fact::helper;\n",
            None,
        ),
        (
            "reexported type",
            "mod attention;\npub use attention::Attention;\n",
            "use crate::fact::Attention;\n",
            None,
        ),
        (
            "path into the file behind a reexport",
            "mod attention;\npub use attention::attention;\n",
            "use crate::fact::attention::helper;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
        (
            "glob reexport",
            "mod attention;\npub use attention::*;\n",
            "use crate::fact::attention;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
        (
            "module reexport",
            "mod attention;\npub use attention;\n",
            "use crate::fact::attention;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
        (
            "self module reexport",
            "mod attention;\npub use self::attention;\n",
            "use crate::fact::attention;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
        (
            "crate visible reexport",
            "mod attention;\npub(crate) use attention::attention;\n",
            "use crate::fact::attention;\n",
            None,
        ),
        (
            "renamed reexport",
            "mod attention;\nmod score;\npub use score::rank as attention;\n",
            "use crate::fact::attention;\n",
            None,
        ),
        (
            "reexport inside an inline module",
            "mod attention;\nmod inner {\n    pub use super::attention::attention;\n}\n",
            "use crate::fact::attention;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
        (
            "private use",
            "mod attention;\nuse attention::attention;\n",
            "use crate::fact::attention;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
        (
            "super reexport of an item",
            "mod attention;\npub use super::fact::attention::attention;\n",
            "use crate::fact::attention;\n",
            None,
        ),
        (
            "leading colon reexport",
            "mod attention;\npub use ::std::mem::drop as attention;\n",
            "use crate::fact::attention;\n",
            None,
        ),
        (
            "grouped leading colon reexport",
            "mod attention;\npub use ::std::mem::{drop as attention};\n",
            "use crate::fact::attention;\n",
            None,
        ),
        (
            "external path mirrored by a local file",
            "mod attention;\npub use ::std::mem::drop as renamed;\n",
            "use crate::fact::renamed;\n",
            None,
        ),
        (
            "external crate named like the child file",
            "mod attention;\npub use ::attention;\n",
            "use crate::fact::attention;\n",
            None,
        ),
        (
            "item reexported under an alias through a module name",
            "mod attention;\npub use self::attention as renamed;\npub use self::renamed::helper as public_helper;\n",
            "use crate::fact::public_helper;\n",
            None,
        ),
        (
            "super module reexport",
            "mod attention;\npub use super::fact::attention;\n",
            "use crate::fact::attention;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
        (
            "crate module reexport",
            "mod attention;\npub use crate::fact::attention;\n",
            "use crate::fact::attention;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
        (
            "reexport of a deeper name",
            "mod attention;\npub use attention::attention;\npub use attention::helper;\n",
            "use crate::fact::attention::helper;\n",
            Some("ticket:src/fact/attention.rs"),
        ),
    ];

    for (case, entry, ticket, key) in cases {
        let root = alias_project(entry, &[], "", ticket);
        let problems = problems(root.path());

        assert_reexport(&problems, key, case);
    }
}

#[test]
fn nested_reexports_decide_whether_the_child_file_is_private() {
    let cases = [
        (
            "reexport from a public file",
            &["src/config/models.rs"][..],
            &[
                (
                    "src/config/models.rs",
                    "pub mod internal;\npub use internal::X;\n",
                ),
                ("src/config/models/internal.rs", "pub struct X;\n"),
                ("src/config/models/X.rs", "pub struct X;\n"),
            ][..],
            "use crate::config::models::X;\n",
            None,
        ),
        (
            "reexport from a private file",
            &[][..],
            &[
                (
                    "src/config/models.rs",
                    "pub mod internal;\npub use internal::X;\n",
                ),
                ("src/config/models/internal.rs", "pub struct X;\n"),
                ("src/config/models/X.rs", "pub struct X;\n"),
            ][..],
            "use crate::config::models::X;\n",
            Some("ticket:src/config/models/X.rs"),
        ),
        (
            "reexport in a non-exporting file",
            &[][..],
            &[
                (
                    "src/config/models.rs",
                    "pub mod internal;\npub use internal::X;\n",
                ),
                ("src/config/models/internal.rs", "pub struct X;\n"),
            ][..],
            "use crate::config::models::internal::X;\n",
            Some("ticket:src/config/models/internal.rs"),
        ),
        (
            "super module reexport from a public file",
            &["src/config/models.rs"][..],
            &[
                (
                    "src/config/models.rs",
                    "pub mod internal;\npub use super::models::internal;\n",
                ),
                ("src/config/models/internal.rs", "pub struct X;\n"),
            ][..],
            "use crate::config::models::internal;\n",
            Some("ticket:src/config/models/internal.rs"),
        ),
    ];

    for (case, public, files, ticket, key) in cases {
        let root = nested_reexport_project(public, files, ticket);
        let problems = problems(root.path());

        assert_reexport(&problems, key, case);
    }
}

fn assert_reexport(problems: &[Problem], key: Option<&str>, case: &str) {
    match key {
        Some(key) => assert_problem(
            problems,
            "src/ticket.rs",
            Rule::ModulePrivateFile,
            key,
            case,
        ),
        None => assert!(problems.is_empty(), "{case}: {problems:?}"),
    }
}

fn nested_reexport_project(
    public: &[&str],
    files: &[(&str, &str)],
    ticket: &str,
) -> tempfile::TempDir {
    let public_row = if public.is_empty() {
        String::new()
    } else {
        let paths: Vec<String> = public.iter().map(|path| format!("\"{path}\"")).collect();

        format!("public = [{}]\n", paths.join(", "))
    };
    let table = format!(
        "[modules.config]\nentry = \"src/config.rs\"\n{public_row}\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n"
    );
    let mut project_files: Vec<(&str, &str)> = vec![("src/config.rs", "pub mod models;\n")];
    project_files.extend(files.iter().copied());
    project_files.push(("src/ticket.rs", ticket));

    project(&project_files, &table)
}

#[test]
fn a_long_chain_of_bare_aliases_resolves_without_repeating_work() {
    let links = 26;
    let mut fact_entry = "pub mod attention;\npub use attention as a1;\n".to_string();

    for link in 2..=links {
        fact_entry.push_str(&format!("pub use a{} as a{link};\n", link - 1));
    }

    let ticket = format!("use crate::fact::a{links}::helper;\n");
    let root = alias_project(&fact_entry, &[], "", &ticket);

    let problems = problems(root.path());

    assert_keys(&problems, &[ATTENTION], "bare chain");
}

#[test]
fn super_past_the_crate_root_reaches_no_file() {
    let root = alias_project(
        "pub mod attention;\npub use super::super::attention as renamed;\n",
        &[],
        "",
        RENAMED_HELPER,
    );

    let problems = problems(root.path());

    assert_keys(&problems, &[], "super past the crate root");
}

#[test]
fn an_alias_to_another_row_is_not_walked_from_the_entry() {
    let root = alias_project(
        "pub use crate::other::attention as renamed;\n",
        &[("src/other.rs", "pub fn attention() {}\n")],
        "\n[modules.other]\nentry = \"src/other.rs\"\n",
        RENAMED_HELPER,
    );

    let problems = problems(root.path());

    let found: Vec<_> = problems
        .iter()
        .map(|problem| (problem.path.as_str(), problem.rule, problem.key.as_deref()))
        .collect();

    assert_eq!(
        found,
        [("src/fact.rs", Rule::ModuleImports, Some("fact:other"))],
        "alias to another row"
    );
}

const CRATE_ROOT_ALIAS: &str = "pub(crate) use crate as renamed;\nmod attention;\n";
const CRATE_ROOT_ALIAS_HELPER: &str = "use crate::fact::renamed::fact::attention::helper;\n";

#[test]
fn crate_root_alias_use_reports_the_private_file_it_reaches() {
    let root = alias_project(CRATE_ROOT_ALIAS, &[], "", CRATE_ROOT_ALIAS_HELPER);

    let problems = problems(root.path());

    assert_keys(&problems, &[ATTENTION], "crate root alias");
}

#[test]
fn crate_root_and_parent_alias_cases_decide_the_reached_private_file() {
    assert_cases([
        case(
            "super alias",
            "pub use super as up;\nmod attention;\n",
            "use crate::fact::up::fact::attention::helper;\n",
            &[ATTENTION],
        ),
        case(
            "super::super from an inline module",
            "mod attention;\npub mod inner { pub use super::super as up; }\n",
            "use crate::fact::inner::up::fact::attention::helper;\n",
            &["fact:crate", ATTENTION],
        ),
        case(
            "twice around",
            CRATE_ROOT_ALIAS,
            "use crate::fact::renamed::fact::renamed::fact::attention::helper;\n",
            &[ATTENTION],
        ),
        case(
            "bare target through the alias",
            "pub(crate) use crate as root;\nmod attention;\npub use root::fact::attention as renamed;\n",
            "use crate::fact::renamed::helper;\n",
            &[ATTENTION],
        ),
        case(
            "leaves the row",
            CRATE_ROOT_ALIAS,
            "use crate::fact::renamed::other::helper;\n",
            &[],
        ),
        case(
            "terminal alias",
            CRATE_ROOT_ALIAS,
            "use crate::fact::renamed;\n",
            &[],
        ),
        Case {
            extra_rows: "\n[modules.attention]\nentry = \"src/fact/attention.rs\"\n",
            ..case(
                "into a deeper row",
                CRATE_ROOT_ALIAS,
                CRATE_ROOT_ALIAS_HELPER,
                &[],
            )
        },
    ]);
}

#[test]
fn an_alias_to_a_shorter_path_outside_a_nested_row_is_not_an_ancestor() {
    let files = [
        ("src/fact/attention.rs", "pub mod inner;\n"),
        (
            "src/fact/attention/inner.rs",
            "mod secret;\npub use crate::other as secret;\n",
        ),
        ("src/fact/attention/inner/secret.rs", "pub fn helper() {}\n"),
        (
            "src/ticket.rs",
            "use crate::fact::attention::inner::secret;\n",
        ),
    ];
    let table = "[modules.attention]\nentry = \"src/fact/attention.rs\"\npublic = [\"src/fact/attention/inner.rs\"]\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"attention\"]\n";
    let root = project(&files, table);

    let private_files: Vec<_> = problems(root.path())
        .into_iter()
        .filter(|problem| problem.rule == Rule::ModulePrivateFile)
        .collect();

    assert_keys(&private_files, &[], "alias to a path outside the row");
}

#[test]
fn fan_in_alias_chains_resolve_without_exponential_work() {
    let mut fan_in =
        "pub mod attention;\npub use self::attention as a0;\npub use self::attention as b0;\n"
            .to_string();

    for level in 1..=64 {
        let previous = level - 1;
        fan_in.push_str(&format!(
            "pub use self::b{previous} as a{level};\npub use self::b{previous} as b{level};\npub use self::a{previous} as b{level};\n"
        ));
    }

    let closed = format!("{fan_in}pub use self::b64 as b0;\n");
    let self_cycles: String = (1..=64)
        .map(|level| {
            format!("pub use self::a{level} as a{level};\npub use self::b{level} as b{level};\n")
        })
        .collect();
    let looped = format!("{fan_in}{self_cycles}");
    let cases = [
        ("fan-in chain", fan_in, vec![ATTENTION]),
        ("fan-in chain closed into a cycle", closed, Vec::new()),
        (
            "fan-in chain with a self-cycle per level",
            looped,
            vec![ATTENTION],
        ),
    ];

    for (name, fact_entry, keys) in cases {
        let ticket = "use crate::fact::b64::helper;\n";
        let problems = problems_within_timeout(name, fact_entry, &[], ticket);

        assert_keys(&problems, &keys, name);
    }
}

#[test]
fn a_use_after_an_exhausted_use_resolves_as_if_it_ran_alone() {
    let levels = 16;
    let mut fact_entry = "pub mod attention;\n".to_string();

    for prefix in ["x", "y"] {
        fact_entry.push_str(&format!(
            "pub use ::std::mem as {prefix}a0;\npub use ::std::mem as {prefix}b0;\n"
        ));

        for level in 1..=levels {
            let previous = level - 1;
            fact_entry.push_str(&format!(
                "pub use self::{prefix}b{previous} as {prefix}a{level};\npub use self::{prefix}b{previous} as {prefix}b{level};\npub use self::{prefix}a{previous} as {prefix}b{level};\n"
            ));
        }

        fact_entry.push_str(&format!("pub use self::{prefix}b{levels} as {prefix}b0;\n"));
    }

    fact_entry.push_str(&format!(
        "pub use self::xb{levels} as top;\npub use self::late as top;\npub use self::yb{levels} as late;\npub use self::attention as late;\n"
    ));

    let exhausting = "use crate::fact::top::helper;\n";
    let second = "use crate::fact::late::helper;\n";
    let cases = [
        ("exhausting use alone", exhausting.to_string(), Vec::new()),
        ("second use alone", second.to_string(), vec![ATTENTION]),
        (
            "second use after the exhausting use",
            format!("{exhausting}{second}"),
            vec![ATTENTION],
        ),
    ];

    for (name, ticket, keys) in cases {
        let root = alias_project(&fact_entry, &[], "", &ticket);
        let problems = problems(root.path());

        assert_keys(&problems, &keys, name);
    }
}

const INNER: &str = "ticket:src/fact/attention/inner.rs";
const INNER_FILES: &[(&str, &str)] = &[
    ("src/fact/attention.rs", "pub mod inner;\n"),
    ("src/fact/attention/inner.rs", "pub fn helper() {}\n"),
];
const GLOB_ENTRY: &str = "mod attention;\npub use attention::*;\n";
const INNER_HELPER: &str = "use crate::fact::inner::helper;\n";

#[test]
fn glob_reexport_use_reports_the_private_file_it_reaches() {
    let root = alias_project(GLOB_ENTRY, INNER_FILES, "", INNER_HELPER);

    let problems = problems(root.path());

    assert_keys(&problems, &[INNER], "glob reexport");
}

#[test]
fn a_restricted_glob_does_not_reach_the_private_file() {
    let cases = [
        ("pub(self)", "pub(self) use attention::*;\n", false),
        (
            "pub(in crate::fact)",
            "pub(in crate::fact) use attention::*;\n",
            false,
        ),
        ("pub", "pub use attention::*;\n", true),
    ];

    for (name, glob, reaches) in cases {
        let fact_entry = format!("mod attention;\n{glob}");
        let root = alias_project(&fact_entry, INNER_FILES, "", INNER_HELPER);
        let problems = problems(root.path());

        if reaches {
            assert_keys(&problems, &[INNER], name);
        } else {
            assert_keys(&problems, &[], name);
        }
    }
}

#[test]
fn glob_reexport_cases_decide_the_reached_private_file() {
    assert_cases([
        Case {
            extra_files: &[("src/fact/attention.rs", "pub fn attention() {}\n")],
            ..case(
                "E1 name bound only by an export of the glob target",
                "mod attention;\nmod hub { pub use super::attention as inner; }\npub use hub::*;\n",
                INNER_HELPER,
                &[ATTENTION],
            )
        },
        Case {
            extra_files: INNER_FILES,
            ..case(
                "G2 terminal glob name",
                GLOB_ENTRY,
                "use crate::fact::inner;\n",
                &[INNER],
            )
        },
        Case {
            extra_files: &[
                ("src/fact/attention.rs", "mod inner;\n"),
                ("src/fact/attention/inner.rs", "pub fn helper() {}\n"),
            ],
            ..case("G3 private mod", GLOB_ENTRY, INNER_HELPER, &[])
        },
        Case {
            extra_files: &[
                ("src/fact/attention.rs", "pub(crate) mod inner;\n"),
                ("src/fact/attention/inner.rs", "pub fn helper() {}\n"),
            ],
            ..case("G4 pub(crate) mod", GLOB_ENTRY, INNER_HELPER, &[INNER])
        },
        Case {
            extra_files: &[(
                "src/fact/attention.rs",
                "pub mod inner { pub fn helper() {} }\n",
            )],
            ..case(
                "G5 public inline module",
                GLOB_ENTRY,
                INNER_HELPER,
                &[ATTENTION],
            )
        },
        Case {
            extra_files: &[(
                "src/fact/attention.rs",
                "mod inner { pub fn helper() {} }\n",
            )],
            ..case("G6 private inline module", GLOB_ENTRY, INNER_HELPER, &[])
        },
        Case {
            extra_files: INNER_FILES,
            ..case(
                "G7 explicit export shadows the glob",
                "mod attention;\npub mod score;\npub use attention::*;\npub use self::score as inner;\n",
                INNER_HELPER,
                &["ticket:src/fact/score.rs"],
            )
        },
        Case {
            extra_files: INNER_FILES,
            ..case(
                "G8 explicit export declared first",
                "mod attention;\npub mod score;\npub use self::score as inner;\npub use attention::*;\n",
                INNER_HELPER,
                &["ticket:src/fact/score.rs"],
            )
        },
        Case {
            extra_files: &[
                ("src/fact/attention.rs", "pub mod inner;\n"),
                ("src/fact/attention/inner.rs", "pub fn helper() {}\n"),
                ("src/fact/inner.rs", "pub fn helper() {}\n"),
            ],
            ..case(
                "G9 child file shadows the glob",
                "mod attention;\npub mod inner;\npub use attention::*;\n",
                INNER_HELPER,
                &["ticket:src/fact/inner.rs"],
            )
        },
        Case {
            extra_files: INNER_FILES,
            ..case(
                "G10 entry inline module shadows the glob",
                "mod attention;\npub use attention::*;\npub mod inner { pub fn helper() {} }\n",
                INNER_HELPER,
                &[],
            )
        },
        Case {
            extra_files: INNER_FILES,
            ..case(
                "G11 glob of a glob",
                "mod attention;\npub mod outer { pub use super::attention::*; }\npub use outer::*;\n",
                INNER_HELPER,
                &[INNER],
            )
        },
        Case {
            extra_files: INNER_FILES,
            ..case(
                "G12 glob inside a group",
                "mod attention;\npub mod score;\npub use self::{score, attention::*};\n",
                INNER_HELPER,
                &[INNER],
            )
        },
        case(
            "G13 external glob",
            "pub use ::std::*;\n",
            INNER_HELPER,
            &[],
        ),
        case(
            "G14 unknown bare glob",
            "pub use serde::*;\n",
            INNER_HELPER,
            &[],
        ),
        case(
            "G15 glob over the crate root",
            "mod attention;\npub use crate::*;\n",
            "use crate::fact::fact::attention::helper;\n",
            &["fact:*", ATTENTION],
        ),
        Case {
            extra_files: &[
                ("src/fact/attention.rs", "mod hidden;\npub use hidden::*;\n"),
                ("src/fact/attention/hidden.rs", "pub mod inner;\n"),
                ("src/fact/attention/hidden/inner.rs", "pub fn helper() {}\n"),
            ],
            ..case(
                "G16 glob in a private file",
                GLOB_ENTRY,
                INNER_HELPER,
                &["ticket:src/fact/attention/hidden/inner.rs"],
            )
        },
        Case {
            extra_files: INNER_FILES,
            ..case(
                "G17 glob over an inline module's parent",
                "mod attention;\npub use attention::*;\npub mod first { pub use super::*; }\n",
                "use crate::fact::first::inner::helper;\n",
                &[INNER],
            )
        },
    ]);
}

#[test]
fn same_name_inline_modules_merge_their_globs_and_public_modules() {
    assert_cases([
        Case {
            extra_files: INNER_FILES,
            ..case(
                "M1 glob in the last of two inline modules",
                "mod attention;\n#[cfg(not(a))]\npub mod first {}\n#[cfg(a)]\npub mod first { pub use super::attention::*; }\n",
                "use crate::fact::first::inner::helper;\n",
                &[INNER],
            )
        },
        Case {
            extra_files: &[(
                "src/fact/attention.rs",
                "#[cfg(not(a))]\npub mod outer {}\n#[cfg(a)]\npub mod outer { pub mod inner { pub fn helper() {} } }\n",
            )],
            ..case(
                "M2 pub mod in the last of two inline modules",
                "mod attention;\npub use attention::outer::*;\n",
                INNER_HELPER,
                &[ATTENTION],
            )
        },
    ]);
}

#[test]
fn glob_reexport_cycles_terminate() {
    let first_second = "pub mod first { pub use super::second::*; }\npub mod second { pub use super::first::*; }\n";
    let cases = [
        (
            "Y1 two modules glob each other",
            first_second,
            &[][..],
            "use crate::fact::first::helper;\n",
            Vec::new(),
        ),
        (
            "Y2 other direction",
            first_second,
            &[][..],
            "use crate::fact::second::helper;\n",
            Vec::new(),
        ),
        (
            "Y3 cycle that also binds",
            "mod attention;\npub mod first { pub use super::second::*; pub use super::attention::*; }\npub mod second { pub use super::first::*; }\n",
            INNER_FILES,
            "use crate::fact::second::inner::helper;\n",
            vec![INNER],
        ),
        (
            "Y5 cycle that binds, repeated uses",
            "mod attention;\npub mod first { pub use super::second::*; pub use super::attention::*; }\npub mod second { pub use super::first::*; }\n",
            INNER_FILES,
            "use crate::fact::second::inner::helper;\nuse crate::fact::first::inner::helper as again;\nuse crate::fact::second::inner;\n",
            vec![INNER],
        ),
        (
            "Y4 self glob",
            "pub use self::*;\n",
            &[][..],
            INNER_HELPER,
            Vec::new(),
        ),
    ];

    for (name, fact_entry, extra_files, ticket, keys) in cases {
        let problems = problems_within_timeout(name, fact_entry.to_string(), extra_files, ticket);

        assert_keys(&problems, &keys, name);
    }
}

#[test]
fn fan_in_glob_chains_resolve_without_exponential_work() {
    let mut fan_in = "mod attention;\npub mod m0 { pub use super::attention::*; }\npub mod n0 { pub use super::attention::*; }\n".to_string();

    for level in 1..=64 {
        let previous = level - 1;

        for prefix in ["m", "n"] {
            fan_in.push_str(&format!(
                "pub mod {prefix}{level} {{ pub use super::m{previous}::*; pub use super::n{previous}::*; }}\n"
            ));
        }
    }

    let closed = fan_in.replace(
        "pub mod m0 { pub use super::attention::*; }",
        "pub mod m0 { pub use super::attention::*; pub use super::m64::*; }",
    );
    let cases = [
        ("P3 fan-in", fan_in, vec![INNER]),
        ("P4 closed into a cycle", closed, Vec::new()),
    ];

    for (name, fact_entry, keys) in cases {
        let ticket = "use crate::fact::m64::inner::helper;\n";
        let problems = problems_within_timeout(name, fact_entry, INNER_FILES, ticket);

        assert_keys(&problems, &keys, name);
    }
}

#[test]
fn globs_of_modules_that_bind_nothing_do_not_spend_the_lookup_budget() {
    let mut fact_entry = "mod attention;\nmod empty;\npub use attention::*;\n".to_string();

    for _ in 0..6_000 {
        fact_entry.push_str("pub use empty::*;\n");
    }

    let files: &'static [(&'static str, &'static str)] = &[
        ("src/fact/attention.rs", "pub mod inner;\n"),
        ("src/fact/attention/inner.rs", "pub fn helper() {}\n"),
        ("src/fact/empty.rs", "fn private() {}\n"),
    ];
    let name = "bound-nothing globs";
    let problems = problems_within_timeout(name, fact_entry, files, INNER_HELPER);

    assert_keys(&problems, &[INNER], name);
}

const GLOB_CHAIN_FILES: &[(&str, &str)] = &[
    ("src/fact/mid.rs", "mod attention;\npub use attention::*;\n"),
    ("src/fact/mid/attention.rs", "pub mod inner;\n"),
    ("src/fact/mid/attention/inner.rs", "pub fn helper() {}\n"),
];

#[test]
fn a_glob_in_a_private_file_reports_the_private_file_it_reaches() {
    let root = alias_project(
        "mod mid;\npub use mid::*;\n",
        GLOB_CHAIN_FILES,
        "",
        INNER_HELPER,
    );

    let problems = problems(root.path());

    assert_keys(
        &problems,
        &["ticket:src/fact/mid/attention/inner.rs"],
        "glob in a private file",
    );
}

#[test]
fn a_crate_root_glob_in_the_entry_file_reports_the_private_file_it_reaches() {
    let root = alias_project(
        "pub use super::*;\nmod attention;\n",
        &[("src/fact/attention.rs", "pub fn helper() {}\n")],
        "",
        "use crate::fact::fact::attention::helper;\n",
    );

    let problems = problems(root.path());
    let found: Vec<_> = problems
        .iter()
        .map(|problem| (problem.path.as_str(), problem.rule, problem.key.as_deref()))
        .collect();

    assert_eq!(
        found,
        [
            ("src/fact.rs", Rule::CrateRootPath, Some("fact:*")),
            ("src/ticket.rs", Rule::ModulePrivateFile, Some(ATTENTION)),
        ],
        "crate root glob"
    );
}

#[test]
fn a_value_export_does_not_hide_a_glob_module_of_the_same_name() {
    let root = alias_project(
        "pub use funcs::thing;\npub use modules::*;\nmod funcs;\nmod modules;\n",
        &[
            ("src/fact/funcs.rs", "pub fn thing() {}\n"),
            ("src/fact/modules.rs", "pub mod thing;\n"),
            ("src/fact/modules/thing.rs", "pub fn helper() {}\n"),
        ],
        "",
        "use crate::fact::thing::helper;\n",
    );

    let problems = problems(root.path());

    assert_keys(
        &problems,
        &["ticket:src/fact/modules/thing.rs"],
        "value export and glob module",
    );
}

#[test]
fn an_unresolved_export_hides_the_glob_module_of_the_same_name() {
    let root = alias_project(
        "pub use serde::de as thing;\npub use modules::*;\nmod modules;\n",
        &[
            ("src/fact/modules.rs", "pub mod thing;\n"),
            ("src/fact/modules/thing.rs", "pub fn helper() {}\n"),
        ],
        "",
        "use crate::fact::thing::helper;\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems, Vec::<Problem>::new(), "unresolved export");
}

#[test]
fn a_private_files_export_hides_its_own_glob_module() {
    let files = [
        ("src/fact.rs", "mod hidden;\npub use hidden::*;\n"),
        (
            "src/fact/hidden.rs",
            "pub use alias as x;\npub use globber::*;\n",
        ),
        ("src/fact/hidden/alias.rs", "pub fn helper() {}\n"),
        ("src/fact/hidden/globber.rs", "pub mod x;\n"),
        ("src/fact/hidden/globber/x.rs", "pub fn helper() {}\n"),
        ("src/ticket.rs", "use crate::fact::x::helper;\n"),
    ];
    let table = "[modules.fact]\nentry = \"src/fact.rs\"\npublic = [\"src/fact/hidden/alias.rs\"]\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"fact\"]\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_eq!(problems, Vec::<Problem>::new(), "private file export");
}

#[test]
fn a_private_files_value_export_does_not_hide_its_glob_module() {
    let files = [
        ("src/fact.rs", "mod hidden;\npub use hidden::*;\n"),
        (
            "src/fact/hidden.rs",
            "pub use funcs::thing as x;\npub use globber::*;\n",
        ),
        ("src/fact/hidden/funcs.rs", "pub fn thing() {}\n"),
        ("src/fact/hidden/globber.rs", "pub mod x;\n"),
        ("src/fact/hidden/globber/x.rs", "pub fn helper() {}\n"),
        ("src/ticket.rs", "use crate::fact::x::helper;\n"),
    ];
    let table = "[modules.fact]\nentry = \"src/fact.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"fact\"]\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_keys(
        &problems,
        &["ticket:src/fact/hidden/globber/x.rs"],
        "private file value export",
    );
}

fn problems_within_timeout(
    name: &str,
    fact_entry: String,
    extra_files: &'static [(&'static str, &'static str)],
    ticket: &'static str,
) -> Vec<Problem> {
    let (sender, receiver) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let root = alias_project(&fact_entry, extra_files, "", ticket);

        sender.send(problems(root.path())).ok();
    });

    receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap_or_else(|_| panic!("{name}: the check did not finish in 10 seconds"))
}

fn assert_cases<const COUNT: usize>(cases: [Case; COUNT]) {
    let mut mismatches = Vec::new();

    for case in cases {
        let root = alias_project(
            case.fact_entry,
            case.extra_files,
            case.extra_rows,
            case.ticket,
        );
        let found: Vec<_> = problems(root.path())
            .into_iter()
            .map(|problem| problem.key)
            .collect();
        let expected: Vec<_> = case.keys.iter().map(|key| Some(key.to_string())).collect();

        if found != expected {
            mismatches.push(format!("{}: found {found:?}", case.name));
        }
    }

    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

fn assert_keys(problems: &[Problem], keys: &[&str], name: &str) {
    let found: Vec<_> = problems
        .iter()
        .map(|problem| (problem.path.as_str(), problem.rule, problem.key.as_deref()))
        .collect();
    let expected: Vec<_> = keys
        .iter()
        .map(|key| ("src/ticket.rs", Rule::ModulePrivateFile, Some(*key)))
        .collect();

    assert_eq!(found, expected, "{name}");
}

fn alias_project(
    fact_entry: &str,
    extra_files: &[(&str, &str)],
    extra_rows: &str,
    ticket: &str,
) -> tempfile::TempDir {
    let mut files = vec![
        ("src/fact.rs", fact_entry),
        (
            "src/fact/attention.rs",
            "pub fn attention() {}\npub fn helper() {}\npub struct Attention;\n",
        ),
        ("src/fact/helper.rs", "pub fn helper() {}\n"),
        ("src/fact/Attention.rs", "pub struct Attention;\n"),
        ("src/fact/score.rs", "pub fn rank() {}\n"),
        ("src/fact/public_helper.rs", "pub fn helper() {}\n"),
        ("src/fact/std/mem/drop.rs", "pub fn drop() {}\n"),
        ("src/ticket.rs", ticket),
    ];
    files.extend(extra_files.iter().copied());
    let table = format!(
        "[modules.fact]\nentry = \"src/fact.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"fact\"]\n{extra_rows}"
    );

    project(&files, &table)
}

#[test]
fn a_use_alias_prefixes_a_later_path() {
    let files = [
        ("src/config.rs", "pub mod parse;\n"),
        ("src/config/parse.rs", "pub fn read() {}\n"),
        (
            "src/ticket.rs",
            "use crate::config as cfg;\ncfg::parse::read();\n",
        ),
    ];
    let cases = [
        (
            "without the nested import",
            "imports = [\"config\"]",
            Some("ticket:config.parse"),
        ),
        (
            "with the nested import",
            "imports = [\"config\", \"config.parse\"]",
            None,
        ),
    ];

    for (name, imports, key) in cases {
        let table = format!(
            "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.\"config.parse\"]\nentry = \"src/config/parse.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\n{imports}\n"
        );
        let root = project(&files, &table);
        let problems = problems(root.path());

        match key {
            Some(key) => assert_problem(&problems, "src/ticket.rs", Rule::ModuleImports, key, name),
            None => assert!(problems.is_empty(), "{name}: {problems:?}"),
        }
    }
}

#[test]
fn a_crate_alias_prefixes_a_later_use() {
    let files = [
        (
            "src/ticket.rs",
            "use crate as root;\nuse root::store::read;\n",
        ),
        ("src/store.rs", ""),
    ];
    let table = "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:store",
        "crate alias",
    );
    assert!(problems[0].message.contains("line 2"), "{problems:?}");
}

#[test]
fn a_parent_alias_in_a_nested_file_reaches_the_nested_row() {
    let files = [
        ("src/a.rs", "mod b;\n"),
        (
            "src/a/b.rs",
            "use super as parent;\nparent::other::read();\n",
        ),
        ("src/a/other.rs", "pub fn read() {}\n"),
    ];
    let table =
        "[modules.a]\nentry = \"src/a.rs\"\n\n[modules.\"a.other\"]\nentry = \"src/a/other.rs\"\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/a/b.rs",
        Rule::ModuleImports,
        "a:a.other",
        "parent alias",
    );
    assert!(problems[0].message.contains("line 2"), "{problems:?}");
}

#[test]
fn a_value_path_does_not_reach_the_module_of_the_same_name() {
    let files = [
        ("src/config.rs", "mod parse;\npub fn parse() {}\n"),
        ("src/config/parse.rs", "pub fn read() {}\n"),
        (
            "src/ticket.rs",
            "fn f() {\n    crate::config::parse();\n}\n",
        ),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_use_path_reaches_the_module_of_the_same_name() {
    let files = [
        ("src/config.rs", "mod parse;\npub fn parse() {}\n"),
        ("src/config/parse.rs", "pub fn read() {}\n"),
        ("src/ticket.rs", "use crate::config::parse;\n"),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModulePrivateFile,
        "ticket:src/config/parse.rs",
        "use path to the module",
    );
}

#[test]
fn a_restricted_use_does_not_bind_through_a_public_glob() {
    let cases = [
        ("pub(self)", "pub(self) use self::hidden as api;", &[][..]),
        (
            "pub(crate)",
            "pub(crate) use self::hidden as api;",
            &["ticket:src/fact/hidden.rs"][..],
        ),
        (
            "bare pub",
            "pub use self::hidden as api;",
            &["ticket:src/fact/hidden.rs"][..],
        ),
    ];

    for (name, binding, keys) in cases {
        let fact_entry = format!("mod hidden;\n{binding}\npub mod hub {{ pub use super::*; }}\n");
        let root = alias_project(
            &fact_entry,
            &[("src/fact/hidden.rs", "pub fn read() {}\n")],
            "",
            "use crate::fact::hub::api::read;\n",
        );
        let problems = problems(root.path());

        assert_keys(&problems, keys, name);
    }
}

#[test]
fn a_public_glob_does_not_reach_a_restricted_export_of_a_public_module() {
    let files = [
        (
            "src/fact.rs",
            "mod hidden;\npub(self) use self::hidden as api;\npub mod api;\npub mod hub { pub use super::*; }\n",
        ),
        ("src/fact/hidden.rs", "pub fn read() {}\n"),
        ("src/fact/api.rs", "pub fn read() {}\n"),
        ("src/ticket.rs", "use crate::fact::hub::api::read;\n"),
    ];
    let table = "[modules.fact]\nentry = \"src/fact.rs\"\npublic = [\"src/fact/api.rs\"]\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"fact\"]\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_pub_self_use_does_not_reach_the_private_file() {
    let files = [
        (
            "src/config.rs",
            "mod hidden;\npub(self) use self::hidden as api;\n",
        ),
        ("src/config/hidden.rs", "pub fn read() {}\n"),
        ("src/ticket.rs", "use crate::config::api::read;\n"),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\npublic = [\"src/config.rs\"]\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn an_alias_declared_after_its_use_is_checked() {
    let files = [
        (
            "src/ticket.rs",
            "fn a() {\n    root::config::read();\n}\nuse crate as root;\n",
        ),
        ("src/config.rs", "pub fn read() {}\n"),
    ];
    let table = "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.config]\nentry = \"src/config.rs\"\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:config",
        "forward alias",
    );
    assert!(problems[0].message.contains("line 2"), "{problems:?}");
}

#[test]
fn a_function_local_use_does_not_bind_outside_its_function() {
    let files = [
        (
            "src/ticket.rs",
            "fn a() {\n    use crate as root;\n    root::config();\n}\nfn b() {\n    root::config::read();\n}\n",
        ),
        ("src/config.rs", "pub fn read() {}\n"),
    ];
    let table = "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.config]\nentry = \"src/config.rs\"\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn an_alias_is_not_restarted_in_the_middle_of_a_path() {
    let files = [
        (
            "src/ticket.rs",
            "mod local {\n    pub mod root {\n        pub fn config() {}\n    }\n}\nuse crate as root;\nfn f() {\n    local::root::config();\n}\n",
        ),
        ("src/config.rs", "pub fn read() {}\n"),
    ];
    let table = "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.config]\nentry = \"src/config.rs\"\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn self_rooted_aliases_reach_the_nested_row() {
    let files = [
        ("src/ticket/config.rs", "pub mod read;\n"),
        ("src/ticket/config/read.rs", ""),
    ];
    let cases = [
        (
            "self module alias",
            "use self::config as cfg;\nuse cfg::read;\n",
        ),
        (
            "self root alias",
            "use self as here;\nuse here::config::read;\n",
        ),
    ];
    let table = "[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"ticket.config\"]\n\n[modules.\"ticket.config\"]\nentry = \"src/ticket/config.rs\"\npublic = [\"src/ticket/config.rs\"]\n\n[modules.\"ticket.config.read\"]\nentry = \"src/ticket/config/read.rs\"\n";

    for (name, ticket) in cases {
        let mut project_files = files.to_vec();
        project_files.push(("src/ticket.rs", ticket));
        let root = project(&project_files, table);
        let problems = problems(root.path());

        assert_problem(
            &problems,
            "src/ticket.rs",
            Rule::ModuleImports,
            "ticket:ticket.config.read",
            name,
        );
        assert!(
            problems[0].message.contains("line 2"),
            "{name}: {problems:?}"
        );
    }
}

#[test]
fn pub_super_use_at_the_crate_root_is_crate_visible() {
    let files = [
        (
            "src/config.rs",
            "pub(crate) mod hidden;\npub(super) use self::hidden as api;\n",
        ),
        ("src/config/hidden.rs", "pub fn read() {}\n"),
        ("src/ticket.rs", "use crate::config::api::read;\n"),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModulePrivateFile,
        "ticket:src/config/hidden.rs",
        "pub(super) export",
    );
}

#[test]
fn a_pub_super_use_below_the_crate_root_does_not_reach_the_private_file() {
    let files = [
        ("src/config.rs", "pub mod parse;\n"),
        (
            "src/config/parse.rs",
            "mod hidden;\npub(super) use self::hidden as api;\n",
        ),
        ("src/config/parse/hidden.rs", "pub fn read() {}\n"),
        ("src/ticket.rs", "use crate::config::parse::api::read;\n"),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.\"config.parse\"]\nentry = \"src/config/parse.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\", \"config.parse\"]\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_pub_super_use_in_a_crate_root_inline_module_reaches_the_private_file() {
    let files = [
        (
            "src/lib.rs",
            "mod inner {\n    mod hidden;\n    pub(super) use self::hidden as api;\n}\n",
        ),
        ("src/lib/inner/hidden.rs", "pub fn read() {}\n"),
        ("src/ticket.rs", "use crate::lib::inner::api::read;\n"),
    ];
    let table = "[modules.root]\nentry = \"src/lib.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"root\"]\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModulePrivateFile,
        "ticket:src/lib/inner/hidden.rs",
        "pub(super) in a crate root inline module",
    );
}

#[test]
fn a_pub_super_use_in_a_crate_root_child_inline_module_stays_restricted() {
    let files = [
        (
            "src/config.rs",
            "mod inner {\n    mod hidden;\n    pub(super) use self::hidden as api;\n}\n",
        ),
        ("src/config/inner/hidden.rs", "pub fn read() {}\n"),
        ("src/ticket.rs", "use crate::config::inner::api::read;\n"),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_leading_colon_expression_resolves_in_edition_2015() {
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"edition-2015\"\nversion = \"0.1.0\"\nedition = \"2015\"\n",
        ),
        ("src/config.rs", "pub fn read() {}\n"),
        ("src/ticket.rs", "fn f() {\n    ::config::read();\n}\n"),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:config",
        "leading colon expression",
    );
    assert!(problems[0].message.contains("line 2"), "{problems:?}");
}

#[test]
fn cfg_branches_merge_value_items() {
    let files = [
        (
            "src/config.rs",
            "#[cfg(not(a))]\npub mod inner { pub mod parse; }\n#[cfg(a)]\npub mod inner { pub fn parse() {} }\n",
        ),
        ("src/config/inner/parse.rs", "pub fn read() {}\n"),
        (
            "src/ticket.rs",
            "fn f() {\n    crate::config::inner::parse();\n}\n",
        ),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_value_call_without_a_value_item_uses_the_row() {
    let files = [
        ("src/config.rs", "mod parse;\n"),
        ("src/config/parse.rs", "pub fn read() {}\n"),
        (
            "src/ticket.rs",
            "fn f() {\n    crate::config::parse();\n}\n",
        ),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.\"config.parse\"]\nentry = \"src/config/parse.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:config.parse",
        "no value item",
    );
}

#[test]
fn a_value_call_at_a_row_uses_the_parent_row() {
    let files = [
        ("src/config.rs", "mod parse;\npub fn parse() {}\n"),
        ("src/config/parse.rs", "pub fn read() {}\n"),
        (
            "src/ticket.rs",
            "fn f() {\n    crate::config::parse();\n}\n",
        ),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.\"config.parse\"]\nentry = \"src/config/parse.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_type_after_a_single_colon_is_checked() {
    let files = [
        (
            "src/ticket.rs",
            "fn f() {\n    let _: crate::config::Ty;\n}\n",
        ),
        ("src/config.rs", "pub struct Ty;\n"),
    ];
    let table = "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.config]\nentry = \"src/config.rs\"\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:config",
        "type after a single colon",
    );
    assert!(problems[0].message.contains("line 2"), "{problems:?}");
}

#[test]
fn a_parent_alias_does_not_bind_inside_a_child_module() {
    let files = [
        (
            "src/ticket.rs",
            "use crate as root;\nmod outer {\n    mod root {\n        pub fn config() {}\n    }\n    fn f() {\n        root::config();\n    }\n}\n",
        ),
        ("src/config.rs", "pub fn read() {}\n"),
    ];
    let table = "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.config]\nentry = \"src/config.rs\"\n";
    let root = project(&files, table);

    let problems = problems(root.path());
    assert_eq!(problems, Vec::<Problem>::new(), "{problems:?}");
}

#[test]
fn a_value_call_under_an_unlisted_parent_uses_the_owner_row() {
    let files = [
        ("src/config.rs", "pub mod inner { pub fn parse() {} }\n"),
        ("src/config/inner/parse.rs", "pub fn read() {}\n"),
        (
            "src/ticket.rs",
            "fn f() {\n    crate::config::inner::parse();\n}\n",
        ),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.\"config.inner.parse\"]\nentry = \"src/config/inner/parse.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_leading_colon_after_return_resolves_in_edition_2015() {
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"edition-2015\"\nversion = \"0.1.0\"\nedition = \"2015\"\n",
        ),
        ("src/config.rs", "pub fn read() {}\n"),
        (
            "src/ticket.rs",
            "fn f() {\n    return ::config::read();\n}\n",
        ),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:config",
        "leading colon after return",
    );
    assert!(problems[0].message.contains("line 2"), "{problems:?}");
}

#[test]
fn a_comparison_before_a_leading_colon_is_checked() {
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"edition-2015\"\nversion = \"0.1.0\"\nedition = \"2015\"\n",
        ),
        ("src/config.rs", "pub fn read() -> i32 { 0 }\n"),
        ("src/ticket.rs", "fn f() {\n    0 > ::config::read();\n}\n"),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:config",
        "comparison before a leading colon",
    );
    assert!(problems[0].message.contains("line 2"), "{problems:?}");
}

#[test]
fn a_qualified_type_path_is_not_a_leading_colon() {
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"edition-2015\"\nversion = \"0.1.0\"\nedition = \"2015\"\n",
        ),
        ("src/item.rs", "pub fn read() {}\n"),
        (
            "src/ticket.rs",
            "fn f() {\n    <Type as Trait>::item::read();\n}\n",
        ),
    ];
    let table =
        "[modules.item]\nentry = \"src/item.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\n";
    let root = project(&files, table);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_pub_self_use_reaches_the_private_file_from_a_child() {
    let files = [
        (
            "src/config.rs",
            "mod hidden;\npub(self) use self::hidden as api;\npub mod ticket;\n",
        ),
        ("src/config/hidden.rs", "pub fn read() {}\n"),
        ("src/config/ticket.rs", "use super::api::read;\n"),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\npublic = [\"src/config.rs\"]\n\n[modules.ticket]\nentry = \"src/config/ticket.rs\"\nimports = [\"config\"]\n";
    let root = project(&files, table);

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/config/ticket.rs",
        Rule::ModulePrivateFile,
        "ticket:src/config/hidden.rs",
        "pub(self) child",
    );
}

#[test]
fn an_alias_into_a_deeper_row_keeps_its_self_target() {
    let files = [
        (
            "src/fact.rs",
            "pub mod attention;\npub use self::attention as renamed;\n",
        ),
        (
            "src/fact/attention.rs",
            "pub fn attention() {}\npub fn helper() {}\n",
        ),
        ("src/ticket.rs", "use crate::fact::renamed::helper;\n"),
    ];
    let cases = [
        ("without the fact import", "", Some("fact:attention")),
        ("with the fact import", "imports = [\"attention\"]\n", None),
    ];

    for (name, fact_imports, key) in cases {
        let table = format!(
            "[modules.fact]\nentry = \"src/fact.rs\"\n{fact_imports}\n[modules.attention]\nentry = \"src/fact/attention.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"fact\"]\n"
        );
        let root = project(&files, &table);
        let problems = problems(root.path());

        match key {
            Some(key) => assert_problem(&problems, "src/fact.rs", Rule::ModuleImports, key, name),
            None => assert!(problems.is_empty(), "{name}: {problems:?}"),
        }
    }
}

#[test]
fn bindings_hold_the_checked_file_and_its_inline_scopes_only() {
    let files = [
        (
            "src/config.rs",
            "use crate::config::parse as top;\n\
             mod inner {\n\
                 use crate::config::parse as nested;\n\
                 mod deep { use crate::config::parse as deep_nested; }\n\
             }\n",
        ),
        (
            "src/config/parse.rs",
            "use crate::config as submodule;\npub fn read() {}\n",
        ),
    ];
    let table = "[modules.config]\nentry = \"src/config.rs\"\n";
    let root = project(&files, table);
    let Installation::Valid(selection) = config::read(
        root.path(),
        layer::catalog(),
        &config::FileRead::filesystem(),
    ) else {
        panic!("expected a valid installation");
    };
    let (modules, _) = read_modules(root.path(), &selection.modules).expect("read the modules");
    let src_folders = modules
        .iter()
        .map(|module| module.src_folder.clone())
        .collect();
    let tree = source_files::walk(root.path(), Language::Rust).expect("walk the project");
    let sources = read_sources(&tree, &modules, &src_folders).expect("read the sources");
    let resolver = Resolver::new(&tree, &modules, &sources);
    let config = modules
        .iter()
        .find(|module| module.name == "config")
        .expect("the config row");

    let bindings = resolver.bindings(config, &["config".to_string()]);

    let names = |inline_path: &[&str]| -> Vec<String> {
        let path: Vec<String> = inline_path.iter().map(|part| part.to_string()).collect();
        bindings[&path].keys().cloned().collect()
    };

    assert_eq!(
        bindings.keys().cloned().collect::<Vec<Vec<String>>>(),
        [
            Vec::<String>::new(),
            vec!["inner".to_string()],
            vec!["inner".to_string(), "deep".to_string()],
        ]
    );
    assert_eq!(names(&[]), ["top".to_string()]);
    assert_eq!(names(&["inner"]), ["nested".to_string()]);
    assert_eq!(names(&["inner", "deep"]), ["deep_nested".to_string()]);
}
