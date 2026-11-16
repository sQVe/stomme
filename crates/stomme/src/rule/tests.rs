use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::{check, layer};

const RULES_FILE: &str = "../../shared/rules.md";
const LAYERS_FOLDER: &str = "../../layers";

fn crate_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn guide_rule_ids(guide: &str) -> BTreeSet<String> {
    guide
        .lines()
        .filter_map(|line| line.strip_prefix("- `"))
        .filter_map(|rest| rest.split_once("`:"))
        .map(|(id, _)| id.to_string())
        .collect()
}

#[test]
fn each_layer_guide_lists_exactly_the_rules_its_layer_owns() {
    let mut guides: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for entry in fs::read_dir(crate_path(LAYERS_FOLDER)).expect("list the layers") {
        let folder = entry.expect("read a layer entry").path();
        let name = folder
            .file_name()
            .expect("a layer folder has a name")
            .to_string_lossy()
            .into_owned();
        let guide = fs::read_to_string(folder.join("guide.md")).expect("read a layer guide");

        guides.insert(name, guide_rule_ids(&guide));
    }

    let mut owned: BTreeMap<String, BTreeSet<String>> = guides
        .keys()
        .map(|name| (name.clone(), BTreeSet::new()))
        .collect();

    for rule in Rule::ALL {
        if let Owner::Layer(layer) = rule.metadata().owner {
            owned
                .entry(layer.to_string())
                .or_default()
                .insert(rule.id().to_string());
        }
    }

    assert_eq!(guides, owned);
}

#[test]
fn guide_rule_ids_reads_only_list_items_that_start_with_an_id() {
    let guide = "\
# Guide

- `first-rule`: Do one thing.
- `second-rule`: Do another thing.
- Use `code` without an ID.
See `not-a-rule`: in prose.
";
    let expected: BTreeSet<String> = ["first-rule", "second-rule"]
        .into_iter()
        .map(String::from)
        .collect();

    assert_eq!(guide_rule_ids(guide), expected);
}

#[test]
fn rules_table_equals_the_committed_shared_rules() {
    let path = crate_path(RULES_FILE);
    let rewrite = env::var_os("UPDATE").is_some();
    let is_current =
        compare_or_rewrite_table(&path, &markdown_table(), rewrite).expect("read the rules table");

    assert!(
        is_current,
        "{} differs from the rule catalog; rewrite it with `UPDATE=1 cargo nextest run -p stomme rules_table`",
        path.display()
    );
}

#[test]
fn compare_or_rewrite_table_rejects_a_stale_table_and_leaves_it_unchanged() {
    let folder = tempfile::tempdir().expect("create a temporary directory");
    let path = folder.path().join("rules.md");

    fs::write(&path, "stale\n").expect("write the stale table");

    let is_current = compare_or_rewrite_table(&path, "generated\n", false).expect("read the table");

    assert!(!is_current);
    assert_eq!(
        fs::read_to_string(&path).expect("read the table"),
        "stale\n"
    );
}

#[test]
fn compare_or_rewrite_table_rewrites_a_stale_table_when_asked() {
    let folder = tempfile::tempdir().expect("create a temporary directory");
    let path = folder.path().join("rules.md");

    fs::write(&path, "stale\n").expect("write the stale table");

    let is_current =
        compare_or_rewrite_table(&path, "generated\n", true).expect("rewrite the table");

    assert!(is_current);
    assert_eq!(
        fs::read_to_string(&path).expect("read the table"),
        "generated\n"
    );
}

#[test]
fn compare_or_rewrite_table_rejects_a_missing_table() {
    let folder = tempfile::tempdir().expect("create a temporary directory");
    let path = folder.path().join("rules.md");

    let is_current = compare_or_rewrite_table(&path, "generated\n", false).expect("read the table");

    assert!(!is_current);
    assert!(!path.exists());
}

#[test]
fn each_enforced_rule_fails_its_fail_fixture_and_passes_its_pass_fixture() {
    let unenforced: Vec<Rule> = Rule::ALL
        .iter()
        .copied()
        .filter(|rule| rule.metadata().enforcement.check().is_none())
        .collect();

    for &rule in Rule::ALL {
        let metadata = rule.metadata();

        let Some(check) = metadata.enforcement.check() else {
            continue;
        };

        for (case, reports_the_rule) in [("fail", true), ("pass", false)] {
            let source = crate_path(check.fixtures).join(case);

            assert!(
                source.is_dir(),
                "{} has no {case} fixture at {}",
                metadata.id,
                source.display()
            );

            let root = prepare_fixture(&source);
            let problems = check::run(root.path(), layer::catalog()).expect("check the fixture");
            let reported = problems.iter().any(|problem| problem.rule == rule);
            let unenforced_reported: Vec<&str> = problems
                .iter()
                .filter(|problem| unenforced.contains(&problem.rule))
                .map(|problem| problem.rule.id())
                .collect();

            assert_eq!(
                reported, reports_the_rule,
                "{} {case}: {problems:?}",
                metadata.id
            );
            assert_eq!(
                unenforced_reported,
                Vec::<&str>::new(),
                "{} {case}",
                metadata.id
            );
        }
    }
}

#[test]
fn each_checker_path_names_an_existing_file() {
    for &rule in Rule::ALL {
        let metadata = rule.metadata();

        if let Some(check) = metadata.enforcement.check() {
            let path = crate_path(check.checker);

            assert!(path.is_file(), "{}: {}", metadata.id, path.display());
        }
    }
}

#[test]
fn each_rule_fixture_folder_belongs_to_a_rule() {
    let named: Vec<PathBuf> = Rule::ALL
        .iter()
        .filter_map(|rule| rule.metadata().enforcement.check())
        .map(|check| crate_path(check.fixtures))
        .collect();
    let folder = crate_path("tests/fixtures/rules");

    for entry in fs::read_dir(&folder).expect("list the rule fixtures") {
        let path = entry.expect("read a rule fixture entry").path();

        assert!(named.contains(&path), "{} names no rule", path.display());
    }
}

fn prepare_fixture(source: &Path) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");

    copy_tree(source, root.path());

    if root.path().join(".sync").is_file() {
        crate::sync::run(root.path(), layer::catalog()).expect("sync the fixture");
        apply_after_sync(root.path());
    }

    if root.path().join(".package-pins").is_file() {
        crate::check::write_pinned_fixture(root.path());
    }

    root
}

fn copy_tree(source: &Path, target: &Path) {
    for entry in fs::read_dir(source).expect("read the fixture folder") {
        let entry = entry.expect("read a fixture entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());

        if entry.file_type().expect("read the file type").is_dir() {
            fs::create_dir_all(&destination).expect("create the fixture folder");
            copy_tree(&path, &destination);
        } else {
            fs::copy(&path, &destination).expect("copy the fixture file");
        }
    }
}

fn apply_after_sync(root: &Path) {
    let changes = root.join("after-sync");

    if changes.is_dir() {
        copy_tree(&changes, root);
    }
}
