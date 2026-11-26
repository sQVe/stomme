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
