use std::fs;
use std::path::Path;

use super::check;
use crate::check::TargetBranch;
use crate::rule::Rule;
use crate::test_support::{commit, git};

fn repository(base: &[(&str, &str)]) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");

    git(root.path(), &["init", "-q"]);

    for (path, text) in base {
        write(root.path(), path, text);
    }

    commit(root.path(), "base");
    git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );

    root
}

fn target() -> TargetBranch {
    TargetBranch::git("origin/main".to_string())
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    let parent = path.parent().expect("a file lies inside the root");

    fs::create_dir_all(parent).expect("create the folder");
    fs::write(&path, text).expect("write the file");
}

fn remove(root: &Path, path: &str) {
    fs::remove_file(root.join(path)).expect("remove the file");
}

fn stage(root: &Path) {
    git(root, &["add", "-A"]);
}

fn assert_problem(problems: &[crate::check::Problem], path: &str, named: &str) {
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].path, path, "{problems:?}");
    assert_eq!(problems[0].rule, Rule::WeakenedTest, "{problems:?}");
    assert!(
        problems[0].message.contains(named),
        "{}",
        problems[0].message
    );
}

#[test]
fn added_rust_skip_is_reported_with_its_line() {
    let root = repository(&[("src/lib.rs", "fn helper() {}\n")]);

    write(
        root.path(),
        "src/lib.rs",
        "fn helper() {}\n\n#[ignore]\n#[test]\nfn slow() {}\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "line 3: adds the test skip `#[ignore]`",
    );
}

#[test]
fn added_typescript_skip_is_reported_with_its_line() {
    let root = repository(&[("src/app.test.ts", "it('runs', () => {});\n")]);

    write(
        root.path(),
        "src/app.test.ts",
        "it('runs', () => {});\n\nit.skip('skips', () => {});\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/app.test.ts",
        "line 3: adds the test skip `it.skip(`",
    );
}

#[test]
fn added_rust_suppression_is_reported_with_its_line() {
    let root = repository(&[("src/lib.rs", "fn one() {}\n")]);

    write(
        root.path(),
        "src/lib.rs",
        "fn one() {}\n\n#[expect(dead_code)]\nfn two() {}\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "line 3: adds the suppression `#[expect(`",
    );
}

#[test]
fn added_typescript_suppression_is_reported_with_its_line() {
    let root = repository(&[("src/app.ts", "export const one = 1;\n")]);

    write(
        root.path(),
        "src/app.ts",
        "export const one = 1;\n\n// @ts-expect-error\nconst two: number = '';\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/app.ts",
        "line 3: adds the suppression `// @ts-expect-error`",
    );
}

#[test]
fn marker_inside_a_line_is_not_reported() {
    let root = repository(&[("src/lib.rs", "fn one() {}\n")]);

    write(
        root.path(),
        "src/lib.rs",
        "fn one() {}\nconst MARKER: &str = \"#[ignore]\";\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_eq!(problems, Vec::new(), "{problems:?}");
}

#[test]
fn added_double_plus_line_keeps_the_following_directive_line() {
    let root = repository(&[("src/app.ts", "export const one = 1;\n")]);

    write(
        root.path(),
        "src/app.ts",
        "export const one = 1;\n\n++counter;\n// @ts-ignore\nconst two: number = '';\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/app.ts",
        "line 4: adds the suppression `// @ts-ignore`",
    );
}

#[test]
fn literal_pathspec_keeps_a_glob_like_path_to_its_own_lines() {
    let root = repository(&[
        ("src/[route].ts", "export const route = 1;\n"),
        ("src/r.ts", "export const r = 1;\n"),
    ]);

    write(root.path(), "src/[route].ts", "export const route = 2;\n");
    write(
        root.path(),
        "src/r.ts",
        "export const r = 1;\n// @ts-ignore\nconst broken: number = '';\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/r.ts",
        "line 2: adds the suppression `// @ts-ignore`",
    );
}

#[test]
fn deleted_rust_test_case_is_reported_at_the_base_line() {
    let root = repository(&[("src/lib.rs", "fn helper() {}\n\n#[test]\nfn removed() {}\n")]);

    write(root.path(), "src/lib.rs", "fn helper() {}\n");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `removed` that `origin/main` declares on line 4; keep the case",
    );
}

#[test]
fn deleted_test_with_a_second_attribute_is_reported() {
    let root = repository(&[("src/lib.rs", "#[test]\n#[cfg(unix)]\nfn runs() {}\n")]);

    write(root.path(), "src/lib.rs", "");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `runs` that `origin/main` declares on line 3; keep the case",
    );
}

#[test]
fn deleted_async_tokio_test_is_reported() {
    let root = repository(&[("src/lib.rs", "#[tokio::test]\nasync fn runs() {}\n")]);

    write(root.path(), "src/lib.rs", "");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `runs` that `origin/main` declares on line 2; keep the case",
    );
}

#[test]
fn added_skip_on_an_existing_test_reports_only_the_skip() {
    let root = repository(&[("src/lib.rs", "#[test]\nfn runs() {}\n")]);

    write(
        root.path(),
        "src/lib.rs",
        "#[test]\n#[ignore]\nfn runs() {}\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "line 2: adds the test skip `#[ignore]`",
    );
}

#[test]
fn deleted_typescript_test_case_is_reported_at_the_base_line() {
    let root = repository(&[(
        "src/app.test.ts",
        "it('first', () => {});\nit('second', () => {});\n",
    )]);

    write(root.path(), "src/app.test.ts", "it('first', () => {});\n");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/app.test.ts",
        "deletes the test case `second` that `origin/main` declares on line 2; keep the case",
    );
}

#[test]
fn renamed_test_case_is_reported_as_deleted() {
    let root = repository(&[("src/lib.rs", "#[test]\nfn old_name() {}\n")]);

    write(root.path(), "src/lib.rs", "#[test]\nfn new_name() {}\n");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `old_name` that `origin/main` declares on line 2",
    );
}

#[test]
fn test_case_moved_to_another_file_is_not_reported() {
    let root = repository(&[
        ("src/one.rs", "#[test]\nfn moved() {}\n"),
        ("src/two.rs", ""),
    ]);

    remove(root.path(), "src/one.rs");
    write(root.path(), "src/two.rs", "#[test]\nfn moved() {}\n");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_eq!(problems, Vec::new(), "{problems:?}");
}

#[test]
fn one_added_case_cannot_hide_two_deletions() {
    let root = repository(&[
        ("src/one.rs", "#[test]\nfn smoke() {}\n"),
        ("src/two.rs", "#[test]\nfn smoke() {}\n"),
    ]);

    remove(root.path(), "src/one.rs");
    remove(root.path(), "src/two.rs");
    write(root.path(), "src/three.rs", "#[test]\nfn smoke() {}\n");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].message.contains("`smoke`"),
        "{}",
        problems[0].message
    );
}

#[test]
fn unchanged_test_file_is_reported_as_nothing() {
    let root = repository(&[(
        "src/lib.rs",
        "#[test]\nfn kept() {\n    assert!(true);\n}\n\nfn value() -> u32 {\n    1\n}\n",
    )]);

    write(
        root.path(),
        "src/lib.rs",
        "#[test]\nfn kept() {\n    assert!(true);\n}\n\nfn value() -> u32 {\n    2\n}\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_eq!(problems, Vec::new(), "{problems:?}");
}

#[test]
fn new_test_file_is_not_reported_as_a_deletion() {
    let root = repository(&[("src/lib.rs", "fn one() {}\n")]);

    write(root.path(), "src/new.rs", "#[test]\nfn fresh() {}\n");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_eq!(problems, Vec::new(), "{problems:?}");
}

#[test]
fn explicit_base_resolves_a_revision_expression() {
    let root = repository(&[("src/lib.rs", "#[test]\nfn kept() {}\n")]);

    write(root.path(), "src/lib.rs", "fn other() {}\n");
    commit(root.path(), "second");
    git(root.path(), &["update-ref", "refs/heads/base", "HEAD"]);

    let problems = check(
        root.path(),
        &TargetBranch::git("refs/heads/base~1".to_string()),
    );

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `kept` that `refs/heads/base~1` declares on line 2",
    );
}

#[test]
fn repository_without_a_commit_reports_nothing() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    git(root.path(), &["init", "-q"]);

    let problems = check(root.path(), &target());

    assert_eq!(problems, Vec::new(), "{problems:?}");
}

#[test]
fn file_read_failure_does_not_ask_for_a_fetch() {
    let root = repository(&[("src/lib.rs", "fn one() {}\n")]);

    remove(root.path(), "src/lib.rs");
    fs::create_dir(root.path().join("src/lib.rs")).expect("make the file a folder");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].message.contains("cannot read `src/lib.rs`"),
        "{}",
        problems[0].message
    );
    assert!(
        !problems[0].message.contains("fetch the target branch"),
        "{}",
        problems[0].message
    );
}

#[test]
fn unresolvable_reference_is_one_problem_naming_it() {
    let root = repository(&[("src/lib.rs", "fn one() {}\n")]);

    let problems = check(
        root.path(),
        &TargetBranch::git("origin/missing".to_string()),
    );

    assert_problem(
        &problems,
        "origin/missing",
        "the ref `origin/missing` does not resolve; fetch the target branch and try again",
    );
}

#[test]
fn unborn_head_still_compares_with_a_resolvable_base() {
    let root = repository(&[("src/lib.rs", "#[test]\nfn kept() {}\n")]);

    write(root.path(), "src/lib.rs", "fn other() {}\n");
    stage(root.path());
    git(root.path(), &["symbolic-ref", "HEAD", "refs/heads/unborn"]);

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `kept` that `origin/main` declares on line 2",
    );
}

#[test]
fn unborn_head_with_a_commit_and_no_target_ref_reports() {
    let root = repository(&[("src/lib.rs", "fn one() {}\n")]);

    git(root.path(), &["symbolic-ref", "HEAD", "refs/heads/unborn"]);

    let problems = check(
        root.path(),
        &TargetBranch::git("origin/missing".to_string()),
    );

    assert_problem(
        &problems,
        "origin/missing",
        "the ref `origin/missing` does not resolve; fetch the target branch and try again",
    );
}

#[test]
fn commented_rust_name_keeps_its_identity() {
    let root = repository(&[(
        "src/lib.rs",
        "#[test]\nfn /* note */ first() {}\n\n#[test]\nfn /* note */ second() {}\n",
    )]);

    write(
        root.path(),
        "src/lib.rs",
        "#[test]\nfn /* note */ second() {}\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `first` that `origin/main` declares on line 2",
    );
}

#[test]
fn multiline_attribute_deletion_is_reported() {
    let root = repository(&[(
        "src/lib.rs",
        "#[test]\n#[cfg_attr(\n    unix,\n    allow(dead_code),\n)]\nfn runs() {}\n",
    )]);

    write(root.path(), "src/lib.rs", "");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `runs` that `origin/main` declares on line 6",
    );
}

#[test]
fn multiline_attribute_on_an_existing_test_reports_no_deletion() {
    let root = repository(&[("src/lib.rs", "#[test]\nfn runs() {}\n")]);

    write(
        root.path(),
        "src/lib.rs",
        "#[test]\n#[cfg_attr(\n    unix,\n    allow(dead_code),\n)]\nfn runs() {}\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_eq!(problems, Vec::new(), "{problems:?}");
}

#[test]
fn typescript_escaped_quote_names_stay_distinct() {
    let root = repository(&[(
        "src/app.test.ts",
        "it('can\\'t read', () => {});\nit('can\\'t write', () => {});\n",
    )]);

    write(
        root.path(),
        "src/app.test.ts",
        "it('can\\'t read', () => {});\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/app.test.ts",
        "deletes the test case `can't write` that `origin/main` declares on line 2",
    );
}

#[test]
fn computed_typescript_first_argument_is_not_a_case() {
    let root = repository(&[(
        "src/app.test.ts",
        "it(name, () => expect(value).toBe('x'));\n",
    )]);

    write(root.path(), "src/app.test.ts", "");
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_eq!(problems, Vec::new(), "{problems:?}");
}

#[test]
fn computed_typescript_first_argument_does_not_credit_a_move() {
    let root = repository(&[(
        "src/app.test.ts",
        "it('first', () => {});\nit(name, () => expect(value).toBe('first'));\n",
    )]);

    write(
        root.path(),
        "src/app.test.ts",
        "it(name, () => expect(value).toBe('first'));\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/app.test.ts",
        "deletes the test case `first` that `origin/main` declares on line 1",
    );
}

#[test]
fn kept_same_name_in_another_changed_file_does_not_credit_a_deletion() {
    let root = repository(&[
        ("src/one.rs", "#[test]\nfn smoke() {}\n"),
        ("src/two.rs", "#[test]\nfn smoke() {}\n"),
    ]);

    write(root.path(), "src/one.rs", "");
    write(
        root.path(),
        "src/two.rs",
        "#[test]\nfn smoke() {}\n\nfn added() {}\n",
    );
    stage(root.path());

    let problems = check(root.path(), &target());

    assert_problem(
        &problems,
        "src/one.rs",
        "deletes the test case `smoke` that `origin/main` declares on line 2",
    );
}

#[test]
fn project_folder_inside_a_repository_reports_only_its_own_change() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    git(root.path(), &["init", "-q"]);
    write(root.path(), "outside.rs", "#[test]\nfn outside() {}\n");
    write(root.path(), "project/src/lib.rs", "#[test]\nfn kept() {}\n");
    commit(root.path(), "base");
    git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );

    write(root.path(), "outside.rs", "fn outside_changed() {}\n");
    write(root.path(), "project/src/lib.rs", "fn other() {}\n");
    stage(root.path());

    let project = root.path().join("project");
    let problems = check(project.as_path(), &target());

    assert_problem(
        &problems,
        "src/lib.rs",
        "deletes the test case `kept` that `origin/main` declares on line 2",
    );
}

#[test]
fn project_folder_without_its_own_change_reports_nothing() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    git(root.path(), &["init", "-q"]);
    write(root.path(), ".gitignore", "target/\n");
    write(root.path(), "outside.rs", "#[test]\nfn outside() {}\n");
    commit(root.path(), "base");
    git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );

    write(root.path(), "outside.rs", "fn outside_changed() {}\n");
    write(
        root.path(),
        "target/project/src/lib.rs",
        "#[test]\nfn kept() {}\n",
    );
    stage(root.path());

    let project = root.path().join("target/project");
    let problems = check(project.as_path(), &target());

    assert_eq!(problems, Vec::new(), "{problems:?}");
}
