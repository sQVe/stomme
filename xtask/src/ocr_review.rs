use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use crate::test_support::temporary_directory;

const OCR_REVIEW: &str = include_str!("../../.github/workflows/ocr-review.yml");
const CLASSIFY_STEP: &str = "Classify and resume the review";
const COVERAGE_STEP: &str = "Add the coverage section";
const MODEL_TOKEN: &str = "model-token-value";
const SESSION_RESULT: &str = r#"{"session_id":"session-1"}"#;
const EMPTY_RESULT: &str = "{}";

const STOMME_FAKE: &str = r#"#!/bin/bash
env | sort > "$STOMME_ENV_LOG"
echo "coverage from $2"
echo "stomme: $2: not reviewed (demo)" >&2
case "$2" in
  *ocr-result.json) exit "${STOMME_FIRST_CODE:-0}" ;;
  *ocr-resume.json) exit "${STOMME_RESUME_CODE:-0}" ;;
esac
exit 0
"#;

const OCR_FAKE: &str = r#"#!/bin/bash
env | sort > "$OCR_ENV_LOG"
printf '%s\n' "$*" > "$OCR_ARGUMENTS"
echo '{"session_id":"session-1"}'
exit "${OCR_RESUME_CODE:-0}"
"#;

const JQ_FAKE: &str = r#"#!/bin/bash
for file in "$@"; do :; done
sed -n 's/.*"session_id": *"\([^"]*\)".*/\1/p' "$file"
"#;

const HELPER_STUB: &str = r#"const fs = require('fs');
const SUMMARY_MARKER = '<!-- ocr-summary -->';
async function findSummaryIssueComment() {
  const file = process.env.COMMENT_FILE;
  if (!file || !fs.existsSync(file)) return null;
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}
module.exports = { SUMMARY_MARKER, findSummaryIssueComment };
"#;

const RUNNER_PREFIX: &str = r#"const fs = require('fs');
globalThis.core = {
  info: (line) => fs.appendFileSync(process.env.INFO_LOG, `${line}\n`),
  warning: (line) => fs.appendFileSync(process.env.WARNING_LOG, `${line}\n`),
  summary: { addRaw: () => ({ write: async () => {} }) },
};
globalThis.context = {
  repo: { owner: 'owner', repo: 'repository' },
  payload: { pull_request: { number: 7 } },
};
globalThis.github = {
  rest: {
    issues: {
      updateComment: async ({ body }) => {
        fs.writeFileSync(process.env.UPDATED_BODY, body);
      },
    },
  },
};
"#;

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn step_block(workflow: &str, step: &str, key: &str) -> String {
    let lines: Vec<&str> = workflow.lines().collect();
    let header = format!("      - name: {step}");
    let start = lines
        .iter()
        .position(|line| *line == header)
        .unwrap_or_else(|| panic!("the workflow has the {step} step"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with("      - "))
        .map_or(lines.len(), |offset| start + 1 + offset);
    let key_line = format!("{key}: |");
    let key_index = (start..end)
        .find(|index| lines[*index].trim() == key_line)
        .unwrap_or_else(|| panic!("the {step} step has a {key} block"));
    let key_indent = indentation(lines[key_index]);
    let body: Vec<&str> = lines[key_index + 1..end]
        .iter()
        .copied()
        .take_while(|line| line.trim().is_empty() || indentation(line) > key_indent)
        .collect();
    let content_indent = body
        .iter()
        .find(|line| !line.trim().is_empty())
        .map(|line| indentation(line))
        .expect("the block has content");
    let mut text: Vec<String> = body
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line[content_indent..].to_string()
            }
        })
        .collect();

    while text.last().is_some_and(String::is_empty) {
        text.pop();
    }

    format!("{}\n", text.join("\n"))
}

fn write_executable(path: &Path, text: &str) {
    fs::write(path, text).expect("write the fake command");
    let mut permissions = fs::metadata(path)
        .expect("the fake command exists")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make the fake command executable");
}

fn search_path(folder: &Path) -> String {
    let inherited = std::env::var("PATH").unwrap_or_default();

    format!("{}:{inherited}", folder.display())
}

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn last_output(output: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");

    output
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix))
        .next_back()
        .map(ToString::to_string)
}

struct Classifier {
    folder: TempDir,
}

struct Observation {
    exit: i32,
    stdout: String,
    output: String,
    coverage: Option<String>,
    classifier_env: String,
    ocr_env: Option<String>,
    ocr_arguments: Option<String>,
}

impl Observation {
    fn final_status(&self) -> Option<String> {
        last_output(&self.output, "final")
    }

    fn resumed(&self) -> bool {
        self.output.lines().any(|line| line == "resumed=true")
    }
}

impl Classifier {
    fn new() -> Self {
        let folder = temporary_directory();
        let root = folder.path();
        let bin = root.join("bin");
        let results = root.join("ocr");
        fs::create_dir_all(&bin).expect("create the fake command folder");
        fs::create_dir_all(&results).expect("create the result folder");
        write_executable(&bin.join("stomme"), STOMME_FAKE);
        write_executable(&bin.join("ocr"), OCR_FAKE);
        write_executable(&bin.join("jq"), JQ_FAKE);

        let script = step_block(OCR_REVIEW, CLASSIFY_STEP, "run")
            .replace("/tmp/", &format!("{}/", results.display()));
        fs::write(root.join("classify.sh"), script).expect("write the extracted script");

        Classifier { folder }
    }

    fn run(&self, first: i32, result: Option<&str>, resume: i32, resumed: i32) -> Observation {
        let root = self.folder.path();
        let results = root.join("ocr");
        let result_file = results.join("ocr-result.json");
        let output_file = root.join("output");
        let _ = fs::remove_file(&result_file);
        let _ = fs::remove_file(&output_file);
        let _ = fs::remove_file(results.join("ocr-coverage.md"));
        let _ = fs::remove_file(root.join("stomme-env"));
        let _ = fs::remove_file(root.join("ocr-env"));
        let _ = fs::remove_file(root.join("ocr-arguments"));

        if let Some(result) = result {
            fs::write(&result_file, result).expect("write the result file");
        }

        let completed = Command::new("/bin/bash")
            .arg("-e")
            .arg(root.join("classify.sh"))
            .current_dir(root)
            .env_clear()
            .env("PATH", search_path(&root.join("bin")))
            .env("GITHUB_OUTPUT", &output_file)
            .env("STOMME", root.join("bin").join("stomme"))
            .env("MERGE_BASE", "merge-base")
            .env("HEAD_SHA", "head-sha")
            .env("EFFORT", "medium")
            .env("RANGE_FROM", "range-from")
            .env("MODEL_TOKEN", MODEL_TOKEN)
            .env("STOMME_FIRST_CODE", first.to_string())
            .env("STOMME_RESUME_CODE", resumed.to_string())
            .env("OCR_RESUME_CODE", resume.to_string())
            .env("STOMME_ENV_LOG", root.join("stomme-env"))
            .env("OCR_ENV_LOG", root.join("ocr-env"))
            .env("OCR_ARGUMENTS", root.join("ocr-arguments"))
            .output()
            .expect("run the extracted classifier script");

        Observation {
            exit: completed
                .status
                .code()
                .expect("the classifier script exits normally"),
            stdout: String::from_utf8_lossy(&completed.stdout).into_owned(),
            output: read(&output_file).unwrap_or_default(),
            coverage: read(&results.join("ocr-coverage.md")),
            classifier_env: read(&root.join("stomme-env")).unwrap_or_default(),
            ocr_env: read(&root.join("ocr-env")),
            ocr_arguments: read(&root.join("ocr-arguments")),
        }
    }
}

#[test]
fn classifier_passes_a_complete_first_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(0, Some(SESSION_RESULT), 0, 0);

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("0"));
    assert!(!observation.resumed());
    assert!(
        observation
            .coverage
            .expect("the first result writes coverage")
            .contains("ocr-result.json")
    );
    assert_eq!(
        observation.ocr_arguments, None,
        "a complete result does not resume"
    );
}

#[test]
fn classifier_fails_a_missing_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(0, None, 0, 0);

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(!observation.resumed());
    assert_eq!(observation.coverage, None);
    assert_eq!(observation.ocr_arguments, None);
}

#[test]
fn classifier_warns_and_fails_a_partial_without_a_session() {
    let classifier = Classifier::new();
    let observation = classifier.run(2, Some(EMPTY_RESULT), 0, 0);

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(!observation.resumed());
    assert!(
        observation
            .stdout
            .contains("::warning::The partial review recorded no session to resume.")
    );
    assert!(
        observation
            .coverage
            .expect("the partial result writes coverage")
            .contains("ocr-result.json")
    );
    assert_eq!(observation.ocr_arguments, None);
}

#[test]
fn classifier_keeps_the_first_coverage_when_the_resume_fails() {
    let classifier = Classifier::new();
    let observation = classifier.run(2, Some(SESSION_RESULT), 1, 0);

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(!observation.resumed());
    assert!(
        observation
            .coverage
            .expect("the first result keeps its coverage")
            .contains("ocr-result.json")
    );
    assert!(
        observation
            .ocr_arguments
            .expect("the resume ran")
            .contains("--resume session-1")
    );
}

#[test]
fn classifier_uses_the_complete_resumed_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(2, Some(SESSION_RESULT), 0, 0);

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("0"));
    assert!(observation.resumed());
    assert!(
        observation
            .coverage
            .expect("the resumed result writes coverage")
            .contains("ocr-resume.json")
    );

    let arguments = observation.ocr_arguments.expect("the resume ran");

    assert!(arguments.contains("--from range-from"), "{arguments}");
    assert!(arguments.contains("--to head-sha"), "{arguments}");
    assert!(arguments.contains("--resume session-1"), "{arguments}");
    assert!(arguments.contains("--effort medium"), "{arguments}");
}

#[test]
fn classifier_reports_a_partial_resumed_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(2, Some(SESSION_RESULT), 0, 2);

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("2"));
    assert!(observation.resumed());
}

#[test]
fn classifier_reports_a_failed_resumed_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(2, Some(SESSION_RESULT), 0, 1);

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(observation.resumed());
}

#[test]
fn classifier_keeps_the_model_token_from_the_classifier() {
    let classifier = Classifier::new();
    let observation = classifier.run(2, Some(SESSION_RESULT), 0, 0);

    assert!(
        !observation.classifier_env.contains(MODEL_TOKEN),
        "{}",
        observation.classifier_env
    );
    assert!(
        observation
            .ocr_env
            .expect("the resume ran")
            .contains(MODEL_TOKEN)
    );
}

struct Coverage {
    folder: TempDir,
    workspace: PathBuf,
}

struct CoverageObservation {
    exit: i32,
    updated: Option<String>,
    info: String,
    warnings: String,
}

impl Coverage {
    fn new() -> Self {
        let folder = temporary_directory();
        let workspace = folder.path().join("workspace");
        let helper = workspace
            .join(".ocr-action")
            .join("scripts")
            .join("github-actions")
            .join("post-review-comments.js");
        fs::create_dir_all(helper.parent().expect("the helper has a folder"))
            .expect("create the helper folder");
        fs::write(&helper, HELPER_STUB).expect("write the helper stub");

        let script = step_block(OCR_REVIEW, COVERAGE_STEP, "script")
            .replace("/tmp/", &format!("{}/", folder.path().display()));
        let runner = format!(
            "{RUNNER_PREFIX}\n(async () => {{\n{script}}})().catch((error) => {{\n  console.error(error);\n  process.exit(1);\n}});\n"
        );
        fs::write(folder.path().join("runner.js"), runner).expect("write the runner");

        Coverage { folder, workspace }
    }

    fn run(&self, coverage: Option<&str>, comment: Option<&str>) -> CoverageObservation {
        let root = self.folder.path();
        let coverage_file = root.join("ocr-coverage.md");
        let comment_file = root.join("comment.json");
        let updated_file = root.join("updated.txt");
        let info_file = root.join("info.txt");
        let warning_file = root.join("warning.txt");
        for path in [
            &coverage_file,
            &comment_file,
            &updated_file,
            &info_file,
            &warning_file,
        ] {
            let _ = fs::remove_file(path);
        }

        if let Some(coverage) = coverage {
            fs::write(&coverage_file, coverage).expect("write the coverage file");
        }

        if let Some(body) = comment {
            let comment = serde_json::json!({
                "id": 1,
                "html_url": "https://example.invalid/comment",
                "body": body,
            });
            fs::write(&comment_file, comment.to_string()).expect("write the comment file");
        }

        let completed = Command::new("node")
            .arg(root.join("runner.js"))
            .current_dir(root)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("GITHUB_WORKSPACE", &self.workspace)
            .env("COMMENT_FILE", &comment_file)
            .env("UPDATED_BODY", &updated_file)
            .env("INFO_LOG", &info_file)
            .env("WARNING_LOG", &warning_file)
            .output()
            .expect("run the coverage script with node");

        assert!(
            completed.status.success(),
            "{}",
            String::from_utf8_lossy(&completed.stderr)
        );

        CoverageObservation {
            exit: completed
                .status
                .code()
                .expect("the coverage script exits normally"),
            updated: read(&updated_file),
            info: read(&info_file).unwrap_or_default(),
            warnings: read(&warning_file).unwrap_or_default(),
        }
    }
}

#[test]
fn coverage_inserts_the_block_after_the_summary_marker() {
    let coverage = Coverage::new();
    let observation = coverage.run(
        Some("Covered: src/lib.rs"),
        Some("<!-- ocr-summary -->\nSummary text"),
    );

    assert_eq!(observation.exit, 0);
    assert_eq!(
        observation.updated.as_deref(),
        Some(
            "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\nCovered: src/lib.rs\n<!-- ocr-coverage:end -->\nSummary text"
        )
    );
}

#[test]
fn coverage_inserts_the_block_after_the_run_tag() {
    let coverage = Coverage::new();
    let observation = coverage.run(
        Some("Covered: src/lib.rs"),
        Some("<!-- ocr-summary -->\n<!-- ocr-summary-run: 123 -->\nSummary text"),
    );

    assert_eq!(observation.exit, 0);
    assert_eq!(
        observation.updated.as_deref(),
        Some(
            "<!-- ocr-summary -->\n<!-- ocr-summary-run: 123 -->\n<!-- ocr-coverage:start -->\nCovered: src/lib.rs\n<!-- ocr-coverage:end -->\nSummary text"
        )
    );
}

#[test]
fn coverage_replaces_an_existing_block() {
    let coverage = Coverage::new();
    let observation = coverage.run(
        Some("Covered: src/lib.rs"),
        Some("<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\nold\n<!-- ocr-coverage:end -->\nSummary"),
    );

    assert_eq!(observation.exit, 0);
    assert_eq!(
        observation.updated.as_deref(),
        Some(
            "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\nCovered: src/lib.rs\n<!-- ocr-coverage:end -->\nSummary"
        )
    );
}

#[test]
fn coverage_leaves_the_comment_alone_without_one() {
    let coverage = Coverage::new();
    let observation = coverage.run(Some("Covered: src/lib.rs"), None);

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.updated, None);
    assert!(
        observation
            .warnings
            .contains("No OCR summary comment; the coverage section is only in the job summary.")
    );
}

#[test]
fn coverage_leaves_the_comment_alone_without_coverage() {
    let coverage = Coverage::new();
    let comment = Some("<!-- ocr-summary -->\nSummary text");
    let missing = coverage.run(None, comment);

    assert_eq!(missing.updated, None);
    assert!(
        missing
            .info
            .contains("No coverage file; leaving the summary comment alone.")
    );

    let empty = coverage.run(Some("  \n"), comment);

    assert_eq!(empty.updated, None);
    assert!(
        empty
            .info
            .contains("The coverage file is empty; leaving the summary comment alone.")
    );
}

#[test]
fn coverage_warns_when_the_summary_has_no_marker() {
    let coverage = Coverage::new();
    let observation = coverage.run(Some("Covered: src/lib.rs"), Some("No marker here"));

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.updated, None);
    assert!(
        observation
            .warnings
            .contains("The summary comment has no marker; not inserting the coverage section.")
    );
}
