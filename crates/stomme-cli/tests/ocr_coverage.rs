use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn a_complete_result_passes() {
    let output = stomme(&fixture("complete.json"));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        "### Review coverage\n\nOCR reviewed 1 of 1 selected file.\n"
    );
    assert_eq!(
        stderr_lines(&output),
        vec!["stomme: OCR review complete: reviewed 1 of 1 selected file"]
    );
}

#[cfg(unix)]
#[test]
fn a_result_path_that_is_not_utf8_is_read() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    let root = tempfile::tempdir().expect("create a temporary directory");
    let path = root.path().join(OsStr::from_bytes(b"complete-\xff.json"));
    std::fs::copy(fixture("complete.json"), &path).expect("copy the complete result");

    let output = stomme(&path);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        "### Review coverage\n\nOCR reviewed 1 of 1 selected file.\n"
    );
    assert_eq!(
        stderr_lines(&output),
        vec!["stomme: OCR review complete: reviewed 1 of 1 selected file"]
    );
}

#[test]
fn a_partial_result_fails_and_lists_only_the_unreviewed_file() {
    let output = stomme(&fixture("partial.json"));

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), partial_fragment());
    assert_eq!(
        stderr_lines(&output),
        vec![
            "stomme: OCR review partial: reviewed 5 of 6 selected files",
            "stomme: crates/stomme-cli/src/sync.rs: not reviewed (budget: reached the maximum tool-request rounds without finishing)",
        ]
    );
    assert!(!stdout(&output).contains("crates/stomme-cli/src/owned.rs"));
    assert!(!stdout(&output).contains("crates/stomme-cli/src/check/synced_files.rs"));
}

#[test]
fn a_failed_result_fails_and_lists_every_file() {
    let output = stomme(&fixture("failed.json"));

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stdout(&output),
        concat!(
            "### Review coverage\n",
            "\n",
            "The OCR review failed. OCR reviewed 0 of 1 selected file.\n",
            "\n",
            "Not reviewed:\n",
            "\n",
            "- `main.rs`: `provider`, `provider or subtask request failed`\n",
        )
    );
    assert_eq!(
        stderr_lines(&output),
        vec![
            "stomme: OCR review failed: reviewed 0 of 1 selected file",
            "stomme: main.rs: not reviewed (provider: provider or subtask request failed)",
        ]
    );
}

#[test]
fn a_skipped_result_passes_without_a_model_call() {
    let output = stomme(&fixture("skipped.json"));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        concat!(
            "### Review coverage\n",
            "\n",
            "OCR selected no files to review.\n",
            "\n",
            "<details>\n",
            "<summary>1 file excluded by the review rules</summary>\n",
            "\n",
            "- `main.rs`: `user_exclude`\n",
            "</details>\n",
        )
    );
    assert_eq!(
        stderr_lines(&output),
        vec!["stomme: OCR review skipped: no files selected"]
    );
}

#[test]
fn a_missing_result_file_is_refused() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    let output = stomme_in(root.path(), Path::new("missing.json"));

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");

    let lines = stderr_lines(&output);

    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].starts_with("stomme: could not read missing.json: "),
        "{lines:?}"
    );
}

#[test]
fn a_terminal_state_with_a_line_break_stays_on_one_stderr_line() {
    let bytes = std::fs::read(fixture("failed.json")).expect("read the fixture");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["terminal_state"] = serde_json::json!("failed\n");

    let root = tempfile::tempdir().expect("create a temporary directory");
    let path = root.path().join("failed.json");
    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");

    std::fs::write(&path, edited).expect("write the edited result");

    let output = stomme_in(root.path(), Path::new("failed.json"));

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr_lines(&output),
        vec!["stomme: failed.json status failed differs from manifest terminal state failed\\n"]
    );
}

#[test]
fn a_missing_path_with_a_line_break_stays_on_one_stderr_line() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    let output = stomme_in(root.path(), Path::new("missing\n.json"));

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");

    let lines = stderr_lines(&output);

    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].starts_with("stomme: could not read missing\\n.json: "),
        "{lines:?}"
    );
}

fn stomme(result: &Path) -> Output {
    stomme_in(Path::new("."), result)
}

fn stomme_in(directory: &Path, result: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stomme"))
        .args(["ocr-coverage"])
        .arg(result)
        .current_dir(directory)
        .output()
        .expect("run stomme")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_string)
        .collect()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ocr-results")
        .join(name)
}

fn partial_fragment() -> String {
    concat!(
        "### Review coverage\n",
        "\n",
        "OCR reviewed 5 of 6 selected files. The check fails until every selected file is reviewed.\n",
        "\n",
        "Not reviewed:\n",
        "\n",
        "- `crates/stomme-cli/src/sync.rs`: `budget`, `reached the maximum tool-request rounds without finishing`\n",
        "\n",
        "<details>\n",
        "<summary>12 files excluded by the review rules</summary>\n",
        "\n",
        "- `crates/stomme-cli/src/check/synced_files/tests.rs`: `default_path`\n",
        "- `crates/stomme-cli/src/guides/tests.rs`: `default_path`\n",
        "- `crates/stomme-cli/src/ocr_rules/tests.rs`: `default_path`\n",
        "- `crates/stomme-cli/src/owned/tests.rs`: `default_path`\n",
        "- `crates/stomme-cli/src/sync/tests.rs`: `default_path`\n",
        "- `docs/adr/0002-purpose-and-design.md`: `unsupported_ext`\n",
        "- `docs/adr/0013-consumer-guides.md`: `unsupported_ext`\n",
        "- `docs/adr/0016-open-code-review.md`: `unsupported_ext`\n",
        "- `docs/adr/README.md`: `unsupported_ext`\n",
        "- `docs/development.md`: `unsupported_ext`\n",
        "- `docs/stomme/.sync-record.toml`: `user_exclude`\n",
        "- `docs/stomme/ocr-rules.json`: `user_exclude`\n",
        "</details>\n",
    )
    .to_string()
}
