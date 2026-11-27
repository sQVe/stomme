use super::*;
use crate::check::test_support::lock_text;

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

fn git_lock(qualifier: &str) -> String {
    format!(
        "[[package]]\nname = \"itoa\"\nversion = \"1.0.15\"\nsource = \"git+https://github.com/dtolnay/itoa?tag=1.0.15#e2766b868e4ac1ae2bf5bea1ac43d4c0da23b899\"\n\n[[package]]\nname = \"lock-probe\"\nversion = \"0.1.0\"\ndependencies = [\n \"itoa 1.0.15 ({qualifier})\",\n]\n"
    )
}

fn sparse_lock(qualifier: &str) -> String {
    format!(
        "version = 4\n\n[[package]]\nname = \"probe-dep\"\nversion = \"0.1.0\"\nsource = \"sparse+http://127.0.0.1:37085/#fragment/\"\n\n[[package]]\nname = \"probe-dep\"\nversion = \"0.1.0\"\nsource = \"sparse+http://127.0.0.1:37085/plain/\"\n\n[[package]]\nname = \"probe-project\"\nversion = \"0.1.0\"\ndependencies = [\n \"probe-dep 0.1.0 ({qualifier})\",\n \"probe-dep 0.1.0 (sparse+http://127.0.0.1:37085/plain/)\",\n]\n"
    )
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
fn a_version_qualifier_rejects_the_only_package_at_another_version() {
    let text = lock_text(&[("tikk-cli", "0.0.0", &["foo 0.1.0"]), ("foo", "0.2.0", &[])]);

    let error = parse(&text).expect_err("the version does not match the only foo");

    assert!(error.contains("names no lock package"), "{error}");
}

#[test]
fn a_source_qualifier_rejects_the_only_package_at_another_source() {
    let text = "version = 4\n\n[[package]]\nname = \"tikk-cli\"\nversion = \"0.0.0\"\ndependencies = [\"foo 1.0.0 (registry+https://example.com/other)\"]\n\n[[package]]\nname = \"foo\"\nversion = \"1.0.0\"\nsource = \"registry+https://example.com/index\"\n";

    let error = parse(text).expect_err("the source does not match the only foo");

    assert!(error.contains("names no lock package"), "{error}");
}

#[test]
fn a_source_qualifier_must_be_nonempty_and_parenthesized() {
    let cases = [
        ("foo 1.0.0 source", "is not enclosed in parentheses"),
        ("foo 1.0.0 (source", "is not enclosed in parentheses"),
        ("foo 1.0.0 ()", "source qualifier is empty"),
    ];

    for (entry, expected) in cases {
        let text = lock_text(&[("tikk-cli", "0.0.0", &[entry]), ("foo", "1.0.0", &[])]);

        let error = parse(&text).expect_err(entry);

        assert!(error.contains(expected), "{entry}: {error}");
    }
}

#[test]
fn a_git_source_qualifier_with_the_exact_commit_selects_the_package() {
    let qualifier =
        "git+https://github.com/dtolnay/itoa?tag=1.0.15#e2766b868e4ac1ae2bf5bea1ac43d4c0da23b899";
    let lock = parse(&git_lock(qualifier)).expect("the qualifier repeats the package commit");
    let dependency = lock
        .dependencies(package(&lock, "lock-probe"))
        .first()
        .expect("the dependency resolves");

    assert_eq!(dependency.source.as_deref(), Some(qualifier));
}

#[test]
fn a_git_source_qualifier_without_the_commit_selects_the_package() {
    let lock = parse(&git_lock("git+https://github.com/dtolnay/itoa?tag=1.0.15"))
        .expect("Cargo omits the commit from the dependency qualifier");
    let dependency = lock
        .dependencies(package(&lock, "lock-probe"))
        .first()
        .expect("the dependency resolves");

    assert_eq!(dependency.name, "itoa");
    assert_eq!(
        dependency.source.as_deref(),
        Some(
            "git+https://github.com/dtolnay/itoa?tag=1.0.15#e2766b868e4ac1ae2bf5bea1ac43d4c0da23b899"
        )
    );
}

#[test]
fn a_non_git_source_qualifier_keeps_its_fragment() {
    let qualifier = "sparse+http://127.0.0.1:37085/#fragment/";
    let lock = parse(&sparse_lock(qualifier)).expect("Cargo keeps the fragment in a sparse source");
    let dependencies = lock.dependencies(package(&lock, "probe-project"));
    let sources: Vec<&str> = dependencies
        .iter()
        .filter_map(|package| package.source.as_deref())
        .collect();

    assert_eq!(
        sources,
        [
            "sparse+http://127.0.0.1:37085/#fragment/",
            "sparse+http://127.0.0.1:37085/plain/"
        ]
    );
}

#[test]
fn a_non_git_source_qualifier_without_its_fragment_names_no_package() {
    let error = parse(&sparse_lock("sparse+http://127.0.0.1:37085/"))
        .expect_err("the qualifier drops the fragment from the sparse source");

    assert_eq!(
        error,
        "has the dependency `probe-dep 0.1.0 (sparse+http://127.0.0.1:37085/)`, which names no lock package"
    );
}

#[test]
fn a_source_qualifier_rejects_the_version_filtered_package_at_another_source() {
    let text = "version = 4\n\n[[package]]\nname = \"tikk-cli\"\nversion = \"0.0.0\"\ndependencies = [\"foo 1.0.0 (registry+https://example.com/other)\"]\n\n[[package]]\nname = \"foo\"\nversion = \"1.0.0\"\nsource = \"registry+https://example.com/index\"\n\n[[package]]\nname = \"foo\"\nversion = \"2.0.0\"\nsource = \"registry+https://example.com/index\"\n";

    let error = parse(text).expect_err("the source does not match the version-filtered foo");

    assert!(error.contains("names no lock package"), "{error}");
}

#[test]
fn a_duplicate_package_identity_is_an_error() {
    let text = lock_text(&[
        ("foo", "1.0.0", &["bar"]),
        ("bar", "1.0.0", &[]),
        ("foo", "1.0.0", &[]),
    ]);

    let error = parse(&text).expect_err("the package is listed twice");

    assert!(error.contains("more than once"), "{error}");
    assert!(error.contains("`foo 1.0.0`"), "{error}");
}

#[test]
fn a_duplicate_sourced_package_identity_names_the_source() {
    let text = "version = 4\n\n[[package]]\nname = \"foo\"\nversion = \"1.0.0\"\nsource = \"registry+https://example.com/index\"\n\n[[package]]\nname = \"foo\"\nversion = \"1.0.0\"\nsource = \"registry+https://example.com/index\"\n";

    let error = parse(text).expect_err("the sourced package is listed twice");

    assert!(
        error.contains("`foo 1.0.0 (registry+https://example.com/index)`"),
        "{error}"
    );
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
