use super::*;

fn lock_text(packages: &[(&str, &str, &[&str])]) -> String {
    let mut text = "version = 4\n".to_string();

    for (name, version, dependencies) in packages {
        text.push_str(&format!(
            "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n"
        ));

        if !dependencies.is_empty() {
            let entries: Vec<String> = dependencies
                .iter()
                .map(|dependency| format!("\"{dependency}\""))
                .collect();

            text.push_str(&format!("dependencies = [{}]\n", entries.join(", ")));
        }
    }

    text
}

fn lock(packages: &[(&str, &str, &[&str])]) -> Lock {
    parse(&lock_text(packages)).expect("parse the lock")
}

fn package<'a>(lock: &'a Lock, name: &str) -> &'a Package {
    lock.packages
        .iter()
        .find(|package| package.name == name)
        .unwrap_or_else(|| panic!("the lock has {name}"))
}

fn names(path: &[Package]) -> Vec<&str> {
    path.iter().map(|package| package.name.as_str()).collect()
}

fn is_forbidden(name: &str) -> bool {
    name == "gpui" || name.starts_with("gpui-") || name.starts_with("gpui_")
}

#[test]
fn finds_a_transitive_path_to_a_forbidden_package() {
    let lock = lock(&[
        ("tikk-cli", "0.0.0", &["clap"]),
        ("clap", "4.0.0", &["gpui-component"]),
        ("gpui-component", "0.0.0", &[]),
    ]);

    let path = lock
        .find_path(package(&lock, "tikk-cli"), is_forbidden)
        .expect("find the forbidden package");

    assert_eq!(names(&path), ["tikk-cli", "clap", "gpui-component"]);
}

#[test]
fn finds_no_path_when_no_forbidden_package_is_reachable() {
    let lock = lock(&[
        ("tikk-cli", "0.0.0", &["clap"]),
        ("clap", "4.0.0", &["serde"]),
        ("serde", "1.0.0", &[]),
    ]);

    assert_eq!(
        lock.find_path(package(&lock, "tikk-cli"), is_forbidden),
        None
    );
}

#[test]
fn terminates_on_dependency_cycles() {
    let lock = lock(&[
        ("tikk-cli", "0.0.0", &["a"]),
        ("a", "0.0.0", &["tikk-cli", "gpui"]),
        ("gpui", "0.0.0", &[]),
    ]);

    let path = lock
        .find_path(package(&lock, "tikk-cli"), is_forbidden)
        .expect("find the forbidden package");

    assert_eq!(names(&path), ["tikk-cli", "a", "gpui"]);
}

#[test]
fn distinct_versions_of_one_crate_keep_their_own_dependencies() {
    let lock = lock(&[
        ("tikk-cli", "0.0.0", &["foo 1.0.0"]),
        ("foo", "1.0.0", &["gpui"]),
        ("foo", "2.0.0", &[]),
        ("gpui", "0.0.0", &[]),
    ]);

    let path = lock
        .find_path(package(&lock, "tikk-cli"), is_forbidden)
        .expect("find the forbidden package");

    assert_eq!(names(&path), ["tikk-cli", "foo", "gpui"]);
}

#[test]
fn a_source_selects_one_of_two_packages_with_the_same_name_and_version() {
    let text = "version = 4\n\n[[package]]\nname = \"tikk-cli\"\nversion = \"0.0.0\"\ndependencies = [\"foo 1.0.0 (registry+https://example.com/index)\"]\n\n[[package]]\nname = \"foo\"\nversion = \"1.0.0\"\nsource = \"registry+https://example.com/index\"\ndependencies = [\"gpui\"]\n\n[[package]]\nname = \"foo\"\nversion = \"1.0.0\"\nsource = \"git+https://example.com/foo\"\n\n[[package]]\nname = \"gpui\"\nversion = \"0.0.0\"\n";
    let lock = parse(text).expect("parse the lock");
    let start = package(&lock, "tikk-cli");

    let path = lock
        .find_path(start, is_forbidden)
        .expect("find the forbidden package");

    assert_eq!(names(&path), ["tikk-cli", "foo", "gpui"]);
}

#[test]
fn malformed_lock_reports_each_missing_field() {
    let cases = [
        (
            "missing name",
            "version = 4\n\n[[package]]\nversion = \"0.0.0\"\n",
            "missing field `name`",
        ),
        (
            "missing version",
            "version = 4\n\n[[package]]\nname = \"tikk-cli\"\n",
            "missing field `version`",
        ),
    ];

    for (case, text, expected) in cases {
        let error = parse(text).expect_err(case);

        assert!(error.contains(expected), "{case}: {error}");
    }
}

#[test]
fn ambiguous_dependency_entry_is_an_error() {
    let text = lock_text(&[
        ("tikk-cli", "0.0.0", &["foo"]),
        ("foo", "1.0.0", &[]),
        ("foo", "2.0.0", &[]),
    ]);

    let error = parse(&text).expect_err("the dependency is ambiguous");

    assert!(
        error.contains("matches more than one lock package"),
        "{error}"
    );
}

#[test]
fn dependency_entry_that_names_no_package_is_an_error() {
    let text = lock_text(&[("tikk-cli", "0.0.0", &["missing"])]);

    let error = parse(&text).expect_err("the dependency names no package");

    assert!(error.contains("names no lock package"), "{error}");
}
