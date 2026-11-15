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

fn produces_problems(status: Status) -> bool {
    matches!(status, Status::Enforced | Status::Partial)
}

fn production_sources(folder: &Path, sources: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(folder).expect("list a source folder") {
        let path = entry.expect("read a source folder entry").path();
        let is_test = path.file_name().is_some_and(|name| name == "tests.rs");

        if path.is_dir() {
            production_sources(&path, sources);
        } else if !is_test && path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path).expect("read a source file");

            sources.push((path, text));
        }
    }
}

fn mentions(text: &str, rule: Rule) -> bool {
    let variant = format!("Rule::{rule:?}");

    text.match_indices(&variant).any(|(start, _)| {
        let next = text[start + variant.len()..].chars().next();

        !next.is_some_and(|character| character.is_alphanumeric() || character == '_')
    })
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
    for &rule in Rule::ALL {
        let metadata = rule.metadata();

        if !produces_problems(metadata.status) {
            assert_eq!(metadata.fixtures, None, "{}", metadata.id);
            continue;
        }

        let Some(folder) = metadata.fixtures else {
            panic!("{} is enforced but names no fixtures", metadata.id);
        };

        for (case, reports_the_rule) in [("fail", true), ("pass", false)] {
            let root = crate_path(folder).join(case);

            assert!(
                root.is_dir(),
                "{} has no {case} fixture at {}",
                metadata.id,
                root.display()
            );

            let problems = check::run(&root, layer::catalog()).expect("check the fixture");
            let reported = problems.iter().any(|problem| problem.rule == rule);

            assert_eq!(
                reported, reports_the_rule,
                "{} {case}: {problems:?}",
                metadata.id
            );
        }
    }
}

#[test]
fn each_enforced_rule_has_a_checker_and_no_other_rule_has_one() {
    for &rule in Rule::ALL {
        let metadata = rule.metadata();

        assert_eq!(
            metadata.checker.is_some(),
            produces_problems(metadata.status),
            "{}",
            metadata.id
        );
    }
}

#[test]
fn only_the_checker_of_an_enforced_rule_reports_it() {
    let mut sources = Vec::new();

    production_sources(&crate_path("src"), &mut sources);

    for &rule in Rule::ALL {
        let metadata = rule.metadata();
        let reporters: Vec<PathBuf> = sources
            .iter()
            .filter(|(_, text)| mentions(text, rule))
            .map(|(path, _)| path.clone())
            .collect();
        let expected = if produces_problems(metadata.status) {
            let checker = metadata.checker.expect("an enforced rule has a checker");

            vec![crate_path(checker)]
        } else {
            Vec::new()
        };

        assert_eq!(reporters, expected, "{}", metadata.id);
    }
}

#[test]
fn each_rule_fixture_folder_belongs_to_a_rule() {
    let named: Vec<PathBuf> = Rule::ALL
        .iter()
        .filter_map(|rule| rule.metadata().fixtures)
        .map(crate_path)
        .collect();
    let folder = crate_path("tests/fixtures/rules");

    for entry in fs::read_dir(&folder).expect("list the rule fixtures") {
        let path = entry.expect("read a rule fixture entry").path();

        assert!(named.contains(&path), "{} names no rule", path.display());
    }
}
