use crate::check::Problem;
use crate::check::modules::tests::{assert_problem, problems};
use crate::check::test_support::project;
use crate::rule::Rule;

const PRIVATE_USE: &str = "use crate::config::parse::read;\n";
const PRIVATE_KEY: &str = "store:src/config/parse.rs";

fn store_project(store_files: &[(&str, &str)]) -> tempfile::TempDir {
    let mut files = vec![
        ("src/config.rs", "pub mod parse;\n"),
        ("src/config/parse.rs", "pub fn read() {}\n"),
    ];
    files.extend_from_slice(store_files);

    project(
        &files,
        "[modules.config]\nentry = \"src/config.rs\"\n\n[modules.store]\nentry = \"src/store.rs\"\nimports = [\"config\"]\n",
    )
}

fn assert_store_private_use(store_entry: &str, expected_path: &str, case: &str) {
    let root = store_project(&[("src/store.rs", store_entry)]);

    assert_problem(
        &problems(root.path()),
        expected_path,
        Rule::ModulePrivateFile,
        PRIVATE_KEY,
        case,
    );
}

#[test]
fn a_private_file_use_from_a_production_tests_file_is_reported() {
    let root = store_project(&[
        ("src/store.rs", "pub mod tests;\n"),
        ("src/store/tests.rs", PRIVATE_USE),
    ]);

    assert_problem(
        &problems(root.path()),
        "src/store/tests.rs",
        Rule::ModulePrivateFile,
        PRIVATE_KEY,
        "production tests file",
    );
}

#[test]
fn a_private_file_use_from_an_inline_tests_module_without_cfg_test_is_reported() {
    let entry = format!("mod tests {{\n    {PRIVATE_USE}}}\n");

    assert_store_private_use(&entry, "src/store.rs", "production inline tests module");
}

#[test]
fn a_cfg_test_file_with_another_name_is_test_code() {
    let root = store_project(&[
        ("src/store.rs", "#[cfg(test)]\nmod fixtures;\n"),
        ("src/store/fixtures.rs", PRIVATE_USE),
    ]);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_file_below_a_test_file_is_test_code() {
    let root = store_project(&[
        ("src/store.rs", "#[cfg(test)]\nmod suite;\n"),
        ("src/store/suite.rs", "mod helpers;\n"),
        ("src/store/suite/helpers.rs", PRIVATE_USE),
    ]);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_file_declared_inside_an_inline_test_module_is_test_code() {
    let root = store_project(&[
        (
            "src/store.rs",
            "#[cfg(test)]\nmod outer {\n    mod inner;\n}\n",
        ),
        ("src/store/outer/inner.rs", PRIVATE_USE),
    ]);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_cfg_all_test_module_is_test_code() {
    let entry = format!("#[cfg(all(test, unix))]\nmod suite {{\n    {PRIVATE_USE}}}\n");
    let root = store_project(&[("src/store.rs", &entry)]);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn a_cfg_any_or_not_test_module_is_production_code() {
    let attributes = ["#[cfg(any(test, feature = \"x\"))]", "#[cfg(not(test))]"];

    for attribute in attributes {
        let entry = format!("{attribute}\nmod tests {{\n    {PRIVATE_USE}}}\n");

        assert_store_private_use(&entry, "src/store.rs", attribute);
    }
}

#[test]
fn a_test_fn_outside_a_test_module_is_production_code() {
    let entry = format!("#[test]\nfn reads() {{\n    {PRIVATE_USE}}}\n");

    assert_store_private_use(&entry, "src/store.rs", "test fn");
}

#[test]
fn a_cfg_test_use_outside_a_test_module_is_production_code() {
    let entry = format!("#[cfg(test)]\n{PRIVATE_USE}");

    assert_store_private_use(&entry, "src/store.rs", "cfg(test) use");
}

#[test]
fn a_test_attribute_on_another_item_does_not_mark_the_next_module() {
    let entry = format!("#[cfg(test)]\nfn helper() {{}}\nmod tests {{\n    {PRIVATE_USE}}}\n");

    assert_store_private_use(&entry, "src/store.rs", "cfg(test) fn before mod");
}

#[test]
fn attributes_and_visibility_between_cfg_test_and_mod_keep_the_mark() {
    let entry = format!(
        "#[cfg(test)]\n#[allow(dead_code)]\npub(crate) mod suite {{\n    {PRIVATE_USE}}}\n"
    );
    let root = store_project(&[("src/store.rs", &entry)]);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}

#[test]
fn an_attribute_that_does_not_gate_on_test_alone_leaves_the_file_production_code() {
    let entries = [
        "#[cfg(all(unix, feature = \"x\"))]\nmod helper;\n",
        "#[cfg(all(unix, not(test)))]\nmod helper;\n",
        "#[cfg(all(all(test)))]\nmod helper;\n",
        "#![cfg(test)]\nmod helper;\n",
        "#[cfg(test)]\n#![allow(dead_code)]\nmod helper;\n",
    ];

    for entry in entries {
        let root = store_project(&[
            ("src/store.rs", entry),
            ("src/store/helper.rs", PRIVATE_USE),
        ]);

        assert_problem(
            &problems(root.path()),
            "src/store/helper.rs",
            Rule::ModulePrivateFile,
            PRIVATE_KEY,
            entry,
        );
    }
}

#[test]
fn cfg_predicates_decide_whether_a_declared_file_is_test_code() {
    let cases = [
        ("#[cfg(test,)]", false),
        ("#[cfg(r#test)]", false),
        ("#[cfg(all(not(unix), test))]", false),
        ("#[cfg(all(test = \"value\"))]", true),
        ("#[cfg(all(feature = \"fixtures\", test))]", false),
        ("#[other(test)]", true),
    ];

    for (attribute, reported) in cases {
        let entry = format!("{attribute}\nmod helper;\n");
        let root = store_project(&[
            ("src/store.rs", &entry),
            ("src/store/helper.rs", PRIVATE_USE),
        ]);
        let problems = problems(root.path());

        if reported {
            assert_problem(
                &problems,
                "src/store/helper.rs",
                Rule::ModulePrivateFile,
                PRIVATE_KEY,
                attribute,
            );
        } else {
            assert!(problems.is_empty(), "{attribute}: {problems:?}");
        }
    }
}

#[test]
fn a_production_declaration_overrides_a_test_declaration_of_the_same_file() {
    let entry = "#[cfg(test)]\nmod helper;\n#[cfg(not(test))]\nmod helper;\n";
    let root = store_project(&[
        ("src/store.rs", entry),
        ("src/store/helper.rs", PRIVATE_USE),
    ]);

    assert_problem(
        &problems(root.path()),
        "src/store/helper.rs",
        Rule::ModulePrivateFile,
        PRIVATE_KEY,
        "test and production declarations",
    );
}

#[test]
fn a_production_declaration_overrides_the_mark_a_file_inherits_from_a_test_parent() {
    let entry = "#[cfg(test)]\nmod outer;\n#[cfg(not(test))]\nmod outer {\n    mod inner;\n}\n";
    let root = store_project(&[
        ("src/store.rs", entry),
        ("src/store/outer.rs", "mod inner;\n"),
        ("src/store/outer/inner.rs", PRIVATE_USE),
    ]);

    assert_problem(
        &problems(root.path()),
        "src/store/outer/inner.rs",
        Rule::ModulePrivateFile,
        PRIVATE_KEY,
        "production declaration beside a test parent",
    );
}

#[test]
fn a_file_only_a_test_only_parent_declares_stays_test_code() {
    let root = store_project(&[
        ("src/store.rs", "#[cfg(test)]\nmod outer;\n"),
        ("src/store/outer.rs", "mod inner;\n"),
        ("src/store/outer/inner.rs", PRIVATE_USE),
    ]);

    assert_eq!(problems(root.path()), Vec::<Problem>::new());
}
