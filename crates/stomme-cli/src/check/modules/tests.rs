use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::config::{self, Installation};
use crate::layer;
use crate::test_support::write_file;

fn project(files: &[(&str, &str)], table: &str) -> TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let config = format!(
        "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n\n{table}",
        env!("CARGO_PKG_VERSION")
    );

    write_file(root.path(), "stomme.toml", &config);

    for (path, text) in files {
        write_file(root.path(), path, text);
    }

    root
}

fn problems(root: &Path) -> Vec<Problem> {
    let Installation::Valid(selection) = config::read(root, layer::catalog()) else {
        panic!("expected a valid installation");
    };

    check(root, &selection.modules).expect("check the project")
}

fn assert_problem(problems: &[Problem], path: &str, rule: Rule, key: &str, case: &str) {
    assert_eq!(problems.len(), 1, "{case}: {problems:?}");
    assert_eq!(problems[0].path, path, "{case}");
    assert_eq!(problems[0].rule, rule, "{case}");
    assert_eq!(problems[0].key.as_deref(), Some(key), "{case}");
}

#[test]
fn forbidden_use_is_reported() {
    let root = project(
        &[
            ("src/ticket.rs", "\nuse crate::store::read;\n"),
            ("src/store.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\n",
    );

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:store",
        "forbidden use",
    );
    assert!(problems[0].message.contains("line 2"), "{problems:?}");
    assert!(problems[0].message.contains("`ticket`"), "{problems:?}");
    assert!(problems[0].message.contains("`store`"), "{problems:?}");
}

#[test]
fn unlisted_module_is_reported() {
    let root = project(
        &[("src/ticket.rs", ""), ("src/cache.rs", "")],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, "src/cache.rs");
    assert_eq!(problems[0].rule, Rule::ModuleListed);
    assert_eq!(problems[0].key, None);
}

#[test]
fn allowed_use_is_not_reported() {
    let root = project(
        &[
            ("src/ticket.rs", "use crate::ticket::Id;\n"),
            ("src/fact.rs", "use crate::ticket::Ticket;\n"),
            (
                "src/store.rs",
                "use crate::{fact, ticket::{Ticket, sync}};\n",
            ),
            ("src/sync.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.fact]\nentry = \"src/fact.rs\"\nimports = [\"ticket\"]\n\n[modules.store]\nentry = \"src/store.rs\"\nimports = [\"ticket\", \"fact\", \"sync\"]\n\n[modules.sync]\nentry = \"src/sync.rs\"\nimports = [\"ticket\", \"fact\"]\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn crate_root_item_and_glob_are_reported() {
    let root = project(
        &[
            ("src/ticket.rs", "use crate::Error;\nuse crate::*;\n"),
            ("src/fact.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.fact]\nentry = \"src/fact.rs\"\n",
    );

    let problems = problems(root.path());

    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!(problems[0].rule, Rule::CrateRootPath);
    assert_eq!(problems[0].key.as_deref(), Some("ticket:Error"));
    assert_eq!(problems[1].rule, Rule::CrateRootPath);
    assert_eq!(problems[1].key.as_deref(), Some("ticket:*"));
}

#[test]
fn comments_and_strings_are_ignored() {
    let root = project(
        &[(
            "src/ticket.rs",
            "// use crate::store::read;\n/* crate::store */\nconst A: &str = \"crate::store::read\";\n",
        )],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn group_use_reports_the_forbidden_module() {
    let root = project(
        &[
            ("src/fact.rs", "use crate::{store, ticket::Ticket};\n"),
            ("src/ticket.rs", ""),
            ("src/store.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.fact]\nentry = \"src/fact.rs\"\nimports = [\"ticket\"]\n\n[modules.store]\nentry = \"src/store.rs\"\n",
    );

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/fact.rs",
        Rule::ModuleImports,
        "fact:store",
        "group use",
    );
}

#[test]
fn file_in_an_owner_folder_is_checked_as_part_of_the_module() {
    let root = project(
        &[
            ("src/ticket.rs", "mod ticket_parse;\n"),
            ("src/ticket/ticket_parse.rs", "use crate::store::read;\n"),
            ("src/store.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\n",
    );

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket/ticket_parse.rs",
        Rule::ModuleImports,
        "ticket:store",
        "owner folder file",
    );
}

#[test]
fn lib_rs_is_not_checked() {
    let root = project(
        &[
            (
                "src/lib.rs",
                "mod store;\nmod ticket;\nuse crate::store::read;\n",
            ),
            ("src/store.rs", ""),
            ("src/ticket.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn super_path_that_leaves_the_module_is_checked() {
    let root = project(
        &[
            ("src/ticket.rs", "mod ticket_parse;\n"),
            (
                "src/ticket/ticket_parse.rs",
                "use super::super::store::read;\n",
            ),
            ("src/store.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\n",
    );

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket/ticket_parse.rs",
        Rule::ModuleImports,
        "ticket:store",
        "super path",
    );
}

#[test]
fn super_path_past_the_crate_root_is_ignored() {
    let root = project(
        &[
            ("src/ticket.rs", "mod ticket_parse;\n"),
            (
                "src/ticket/ticket_parse.rs",
                "use super::super::super::store::read;\n",
            ),
            ("src/store.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_bin_file_owned_by_a_module_is_not_checked() {
    let root = project(
        &[
            ("src/bin/tool.rs", "use crate::store::read;\n"),
            ("src/store.rs", ""),
        ],
        "[modules.tool]\nentry = \"src/bin/tool.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_crate_root_file_owned_by_a_module_is_not_checked() {
    let root = project(
        &[
            ("src/lib.rs", "use crate::store::read;\n"),
            ("src/store.rs", ""),
        ],
        "[modules.root]\nentry = \"src/lib.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_bin_file_that_belongs_to_no_module_is_not_listed() {
    let root = project(
        &[("src/bin/tool.rs", "fn main() {}\n"), ("src/ticket.rs", "")],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_folder_without_its_file_is_not_a_crate_root_path() {
    let root = project(
        &[
            ("src/ticket.rs", "use crate::x::y;\n"),
            ("src/x/mod.rs", ""),
        ],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n\n[modules.x]\nentry = \"src/x/mod.rs\"\n",
    );

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModuleImports,
        "ticket:x",
        "folder without its file",
    );
}

#[test]
fn super_path_inside_an_inline_test_module_stays_in_the_module() {
    let root = project(
        &[("src/ticket.rs", "mod tests {\n    use super::*;\n}\n")],
        "[modules.ticket]\nentry = \"src/ticket.rs\"\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn use_past_an_entry_file_reports_the_private_file() {
    let root = project(
        &[
            ("src/config.rs", "pub mod parse;\n"),
            ("src/config/parse.rs", "pub fn read() {}\n"),
            ("src/ticket.rs", "use crate::config::parse::read;\n"),
        ],
        "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n",
    );

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "src/ticket.rs",
        Rule::ModulePrivateFile,
        "ticket:src/config/parse.rs",
        "private file",
    );
}

#[test]
fn use_past_an_entry_file_passes_when_the_file_is_public() {
    let root = project(
        &[
            ("src/config.rs", "pub mod parse;\n"),
            ("src/config/parse.rs", "pub fn read() {}\n"),
            ("src/ticket.rs", "use crate::config::parse::read;\n"),
        ],
        "[modules.config]\nentry = \"src/config.rs\"\npublic = [\"src/config/parse.rs\"]\n\n[modules.ticket]\nentry = \"src/ticket.rs\"\nimports = [\"config\"]\n",
    );

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_module_path_matches_only_modules_in_the_same_crate() {
    let root = project(
        &[
            ("crates/a/src/ticket.rs", "use crate::store::read;\n"),
            ("crates/b/src/store.rs", ""),
        ],
        "[modules.a_ticket]\nentry = \"crates/a/src/ticket.rs\"\n\n[modules.b_store]\nentry = \"crates/b/src/store.rs\"\n",
    );

    let problems = problems(root.path());

    assert_problem(
        &problems,
        "crates/a/src/ticket.rs",
        Rule::CrateRootPath,
        "a_ticket:store",
        "another crate",
    );
}
