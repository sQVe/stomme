use crate::check::Problem;
use crate::check::modules::tests::{assert_problem, problems};
use crate::check::test_support::project;
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
            ..case("alias into a deeper row", ALIAS_ENTRY, RENAMED_HELPER, &[])
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
