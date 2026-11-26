use super::*;
use crate::rule::Rule;
use crate::test_support::write_file;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn project_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");

    for (path, text) in files {
        write_file(root.path(), path, text);
    }

    root
}

fn workflow(name: &str, line: &str) -> (String, String) {
    (
        format!(".github/workflows/{name}"),
        format!("jobs:\n  job:\n    steps:\n      - {line}\n"),
    )
}

#[test]
fn accepts_a_release_commit_with_the_config_version() {
    let (path, text) = workflow(
        "sync.yml",
        &format!("uses: sQVe/stomme/.github/workflows/sync-update.yml@{COMMIT} # v1.2.3"),
    );
    let root = project_with(&[(&path, &text)]);

    assert_eq!(
        check(root.path(), "1.2.3").expect("check the workflows"),
        []
    );
}

#[test]
fn ignores_local_and_other_repository_calls() {
    let (local_path, local_text) =
        workflow("local.yml", "uses: ./.github/workflows/ocr-review.yml");
    let (other_path, other_text) = workflow(
        "other.yml",
        "uses: octo-org/tools/.github/workflows/lint.yml@v1",
    );
    let root = project_with(&[(&local_path, &local_text), (&other_path, &other_text)]);

    assert_eq!(
        check(root.path(), "1.2.3").expect("check the workflows"),
        []
    );
}

#[test]
fn ignores_a_uses_line_inside_a_run_block() {
    let (path, text) = workflow(
        "ci.yml",
        "run: echo \"uses: sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0\"",
    );
    let root = project_with(&[(&path, &text)]);

    assert_eq!(
        check(root.path(), "1.2.3").expect("check the workflows"),
        []
    );
}

#[test]
fn reports_nothing_without_a_workflows_folder() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    assert_eq!(
        check(root.path(), "1.2.3").expect("check the workflows"),
        []
    );
}

#[test]
fn reads_yaml_extension_files() {
    let (path, text) = workflow(
        "ci.yaml",
        "uses: sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0",
    );
    let root = project_with(&[(&path, &text)]);

    let problems = check(root.path(), "1.2.3").expect("check the workflows");

    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].path, ".github/workflows/ci.yaml");
    assert_eq!(problems[0].rule, Rule::WorkflowPin);
}

#[test]
fn refuses_a_comment_version_that_is_not_a_release_version() {
    let cases = [
        ("an empty core", "v-rc1"),
        (
            "a core character that is neither a digit nor a dot",
            "v1.2x3",
        ),
        ("an empty suffix", "v1.2.3-"),
        (
            "a suffix character that is neither a letter, a digit, a dot, nor a dash",
            "v1.2.3-rc_1",
        ),
    ];

    for (name, version) in cases {
        let (path, text) = workflow(
            "ci.yml",
            &format!("uses: sQVe/stomme/.github/workflows/ocr-review.yml@{COMMIT} # {version}"),
        );
        let root = project_with(&[(&path, &text)]);

        let problems = check(root.path(), "1.2.3").expect("check the workflows");

        assert_eq!(problems.len(), 1, "{name}: {problems:?}");
        assert!(
            problems[0].message.contains("pin a release commit"),
            "{name}: {problems:?}"
        );
    }
}

#[test]
fn refuses_a_quoted_call_with_a_trailing_comment() {
    let (path, text) = workflow(
        "ci.yml",
        "uses: \"sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0\" # v1.2.3",
    );
    let root = project_with(&[(&path, &text)]);

    let problems = check(root.path(), "1.2.3").expect("check the workflows");

    assert_eq!(problems.len(), 1);
    assert!(
        problems[0]
            .message
            .contains("quotes the call to `ocr-review.yml`"),
        "{problems:?}"
    );
}

#[test]
fn refuses_a_call_after_a_dash_with_more_than_one_space() {
    let text = "jobs:\n  job:\n    steps:\n      -   uses: sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0\n";
    let root = project_with(&[(".github/workflows/ci.yml", text)]);

    let problems = check(root.path(), "1.2.3").expect("check the workflows");

    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].message.contains("by the moving name `v0.2.0`"),
        "{problems:?}"
    );
}

#[test]
fn names_the_workflow_without_its_trailing_comment() {
    let cases = [
        (
            "unquoted",
            "uses: sQVe/stomme/.github/workflows/ocr-review.yml # v0.2.4",
            "calls `ocr-review.yml` by the moving name ``",
        ),
        (
            "quoted",
            "uses: \"sQVe/stomme/.github/workflows/ocr-review.yml\" # v0.2.4",
            "quotes the call to `ocr-review.yml`",
        ),
    ];

    for (name, line, expected) in cases {
        let (path, text) = workflow("ci.yml", line);
        let root = project_with(&[(&path, &text)]);

        let problems = check(root.path(), "1.2.3").expect("check the workflows");

        assert_eq!(problems.len(), 1, "{name}: {problems:?}");
        assert!(
            problems[0].message.contains(expected),
            "{name}: {problems:?}"
        );
    }
}

#[test]
fn reports_the_same_messages_for_unquoted_calls() {
    let cases = [
        (
            "a tag ref",
            "uses: sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0".to_string(),
            "by the moving name `v0.2.0`",
        ),
        (
            "a commit with no comment",
            format!("uses: sQVe/stomme/.github/workflows/ocr-review.yml@{COMMIT}"),
            "at a commit with no version comment",
        ),
        (
            "a comment with no version",
            format!("uses: sQVe/stomme/.github/workflows/ocr-review.yml@{COMMIT} # notes"),
            "with the comment `# notes`",
        ),
        (
            "a different version",
            format!("uses: sQVe/stomme/.github/workflows/ocr-review.yml@{COMMIT} # v1.2.2"),
            "at v1.2.2; `stomme.toml` has version 1.2.3",
        ),
    ];

    for (name, line, expected) in cases {
        let (path, text) = workflow("ci.yml", &line);
        let root = project_with(&[(&path, &text)]);

        let problems = check(root.path(), "1.2.3").expect("check the workflows");

        assert_eq!(problems.len(), 1, "{name}: {problems:?}");
        assert!(
            problems[0].message.contains(expected),
            "{name}: {problems:?}"
        );
    }
}

#[test]
fn accepts_a_release_candidate_comment_version() {
    let (path, text) = workflow(
        "ci.yml",
        &format!("uses: sQVe/stomme/.github/workflows/ocr-review.yml@{COMMIT} # v1.2.3-rc.1"),
    );
    let root = project_with(&[(&path, &text)]);

    assert_eq!(
        check(root.path(), "1.2.3-rc.1").expect("check the workflows"),
        []
    );
}

#[test]
fn ignores_a_workflow_file_with_another_extension() {
    let (path, text) = workflow(
        "notes.txt",
        "uses: sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0",
    );
    let root = project_with(&[(&path, &text)]);

    assert_eq!(
        check(root.path(), "1.2.3").expect("check the workflows"),
        []
    );
}

#[test]
fn reads_a_call_without_an_at_sign_as_an_empty_moving_name() {
    let (path, text) = workflow(
        "ci.yml",
        "uses: sQVe/stomme/.github/workflows/ocr-review.yml",
    );
    let root = project_with(&[(&path, &text)]);

    let problems = check(root.path(), "1.2.3").expect("check the workflows");

    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].message.contains("by the moving name ``"),
        "{problems:?}"
    );
}

#[cfg(unix)]
#[test]
fn skips_a_symlinked_workflows_folder() {
    let root = project_with(&[]);
    let target = tempfile::tempdir().expect("create a temporary directory");
    let (path, text) = workflow(
        "ci.yml",
        "uses: sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0",
    );

    write_file(target.path(), &path, &text);
    write_file(root.path(), ".github/README.md", "github\n");
    std::os::unix::fs::symlink(
        target.path().join(".github/workflows"),
        root.path().join(".github/workflows"),
    )
    .expect("create the symlink");

    assert_eq!(
        check(root.path(), "1.2.3").expect("check the workflows"),
        []
    );
}

#[cfg(unix)]
#[test]
fn skips_a_symlinked_github_folder() {
    let root = project_with(&[]);
    let target = tempfile::tempdir().expect("create a temporary directory");
    let (path, text) = workflow(
        "ci.yml",
        "uses: sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0",
    );

    write_file(target.path(), &path, &text);
    std::os::unix::fs::symlink(target.path().join(".github"), root.path().join(".github"))
        .expect("create the symlink");

    assert_eq!(
        check(root.path(), "1.2.3").expect("check the workflows"),
        []
    );
}

#[cfg(unix)]
#[test]
fn skips_a_symlinked_workflow_file() {
    let root = project_with(&[]);
    let target = tempfile::tempdir().expect("create a temporary directory");
    let (path, text) = workflow(
        "ci.yml",
        "uses: sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.0",
    );

    write_file(target.path(), &path, &text);
    write_file(root.path(), ".github/workflows/README.md", "workflows\n");
    std::os::unix::fs::symlink(
        target.path().join(&path),
        root.path().join(".github/workflows/ci.yml"),
    )
    .expect("create the symlink");

    assert_eq!(
        check(root.path(), "1.2.3").expect("check the workflows"),
        []
    );
}
