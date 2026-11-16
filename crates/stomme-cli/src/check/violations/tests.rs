use super::*;
use crate::config::Violation;
use crate::rule::Rule;

fn problem(path: &str, key: &str) -> Problem {
    Problem {
        path: path.to_string(),
        rule: Rule::RoleFolder,
        message: format!("reports the folder `{key}`"),
        key: Some(key.to_string()),
    }
}

fn recorded(path: &str, key: &str) -> Violation {
    Violation {
        path: path.to_string(),
        rule: Rule::RoleFolder,
        key: key.to_string(),
    }
}

#[test]
fn recorded_violation_skips_only_its_break() {
    let problems = vec![
        problem("src/domain", "domain"),
        problem("src/domain", "extra"),
    ];
    let recorded = vec![recorded("src/domain", "domain")];

    let remaining = apply(problems, &recorded);

    assert_eq!(remaining.len(), 1, "{remaining:?}");
    assert_eq!(remaining[0].path, "src/domain");
    assert_eq!(remaining[0].key.as_deref(), Some("extra"));
}

#[test]
fn stale_entry_fails_naming_the_entry() {
    let recorded = vec![recorded("src/domain", "domain")];

    let problems = apply(Vec::new(), &recorded);

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, CONFIG_FILE);
    assert_eq!(problems[0].rule, Rule::ViolationStale);
    for named in ["src/domain", "role-folder", "domain"] {
        assert!(
            problems[0].message.contains(named),
            "{}",
            problems[0].message
        );
    }
}

type ReadTarget = fn(&Path, &str) -> TargetRead;

fn target_config(entries: &[(&str, &str, &str)]) -> String {
    let mut text = String::new();

    for (path, rule, key) in entries {
        text.push_str(&format!(
            "[[violations]]\npath = \"{path}\"\nrule = \"{rule}\"\nkey = \"{key}\"\n"
        ));
    }

    text
}

fn one_entry() -> Vec<Violation> {
    vec![recorded("src/domain", "domain")]
}

fn equal_read(_root: &Path, _reference: &str) -> TargetRead {
    TargetRead::Config(target_config(&[("src/domain", "role-folder", "domain")]))
}

fn no_config_read(_root: &Path, _reference: &str) -> TargetRead {
    TargetRead::NoConfig
}

fn unavailable_read(_root: &Path, _reference: &str) -> TargetRead {
    TargetRead::Unavailable("the ref does not resolve".to_string())
}

fn added_read(_root: &Path, _reference: &str) -> TargetRead {
    TargetRead::Config(target_config(&[]))
}

fn removed_read(_root: &Path, _reference: &str) -> TargetRead {
    TargetRead::Config(target_config(&[("src/other", "role-folder", "other")]))
}

fn unparsable_read(_root: &Path, _reference: &str) -> TargetRead {
    TargetRead::Config("not valid TOML\n".to_string())
}

fn unexpected_read(_root: &Path, _reference: &str) -> TargetRead {
    panic!("an empty list must not read the target branch");
}

#[test]
fn growth_fails_for_each_case() {
    let cases: [(&str, Vec<Violation>, ReadTarget, usize); 7] = [
        ("added entry", one_entry(), added_read, 1),
        ("entry removed on the target", one_entry(), removed_read, 1),
        ("unavailable ref", one_entry(), unavailable_read, 1),
        ("unparsable target", one_entry(), unparsable_read, 1),
        ("equal list", one_entry(), equal_read, 0),
        ("no config on the target", one_entry(), no_config_read, 0),
        ("empty list", Vec::new(), unexpected_read, 0),
    ];

    for (case, recorded, read, expected) in cases {
        let target = TargetBranch {
            reference: "origin/main".to_string(),
            read,
        };

        let problems = growth(Path::new("."), &recorded, &target);

        assert_eq!(problems.len(), expected, "{case}: {problems:?}");
        for problem in &problems {
            assert_eq!(problem.path, CONFIG_FILE, "{case}");
            assert_eq!(problem.rule, Rule::ViolationGrowth, "{case}");
            assert!(problem.message.contains("origin/main"), "{case}: {problem}");
        }
        if matches!(case, "added entry" | "entry removed on the target") {
            assert!(
                problems[0].message.contains("src/domain"),
                "{case}: {}",
                problems[0].message
            );
        }
    }
}
