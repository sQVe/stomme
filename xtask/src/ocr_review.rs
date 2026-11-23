use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use crate::test_support::temporary_directory;

const OCR_REVIEW: &str = include_str!("../../.github/workflows/ocr-review.yml");
const OCR_CALLER: &str = include_str!("../../.github/workflows/ocr.yml");
const CLASSIFY_STEP: &str = "Classify and resume the review";
const COVERAGE_STEP: &str = "Add the coverage section";
const REPORT_STEP: &str = "Report the OCR result";
const SESSION_RESULT: &str = r#"{"session_id":"session-1"}"#;
const EMPTY_RESULT: &str = "{}";
const MIXED_RESULT: &str = r#"{
  "status": "partial",
  "session_id": "session-1",
  "manifest": {
    "schema_version": "ocr.run-manifest/v1",
    "terminal_state": "partial",
    "coverage": {
      "selected": [
        {"item_id": "a", "path": "src/a.rs"},
        {"item_id": "b", "path": "src/b.rs"}
      ],
      "completed": [{"item_id": "a", "path": "src/a.rs"}],
      "reused": [],
      "failed": [
        {"item_id": "b", "path": "src/b.rs", "classification": "provider", "reason": "upstream 500"}
      ],
      "waived": [],
      "excluded": []
    }
  }
}"#;
const TIMEOUT_RESULT: &str = r#"{
  "status": "failed",
  "session_id": "session-2",
  "manifest": {
    "schema_version": "ocr.run-manifest/v1",
    "terminal_state": "failed",
    "run_failure": {"classification": "timeout", "reason": "the global deadline elapsed"},
    "coverage": {
      "selected": [{"item_id": "a", "path": "src/a.rs"}],
      "completed": [],
      "reused": [],
      "failed": [{"item_id": "a", "path": "src/a.rs", "classification": "timeout"}],
      "waived": [],
      "excluded": []
    }
  }
}"#;

const MULTILINE_RESULT: &str = r#"{
  "manifest": {
    "run_failure": {"classification": "timeout", "reason": "first line\nsecond line"}
  }
}"#;

const ASSIGNMENT_REASON_RESULT: &str = r#"{
  "manifest": {
    "run_failure": {"classification": "timeout", "reason": "first line\nfinal=0\ncoverage_source=resumed"}
  }
}"#;

const STOMME_FAKE: &str = r#"#!/bin/bash
case "$2" in
  *ocr-resume.json)
    if [ "${STOMME_RESUME_UNUSABLE:-}" = "1" ]; then
      echo "stomme: $2 is not a usable OCR result" >&2
      exit 1
    fi
    ;;
esac
echo "coverage from $2"
echo "stomme: $2: not reviewed (demo)" >&2
case "$2" in
  *ocr-result.json) exit "${STOMME_FIRST_CODE:-0}" ;;
  *ocr-resume.json) exit "${STOMME_RESUME_CODE:-0}" ;;
esac
exit 0
"#;

const OCR_FAKE: &str = r#"#!/bin/bash
printf '%s\n' "$*" > "$OCR_ARGUMENTS"
body="${OCR_RESUME_BODY-}"
if [ -z "$body" ]; then
  body='{"session_id":"session-1"}'
fi
printf '%s\n' "$body"
exit "${OCR_RESUME_CODE:-0}"
"#;

const HELPER_STUB: &str = r#"const SUMMARY_MARKER = '<!-- ocr-summary -->';
module.exports = { SUMMARY_MARKER };
"#;

const RUNNER_PREFIX: &str = r#"const fs = require('fs');
const chunks = [];
globalThis.core = {
  info: (line) => fs.appendFileSync(process.env.INFO_LOG, `${line}\n`),
  warning: (line) => fs.appendFileSync(process.env.WARNING_LOG, `${line}\n`),
  summary: {
    addRaw: (text) => {
      chunks.push(text);
      return {
        write: async () => {
          fs.writeFileSync(process.env.SUMMARY_LOG, chunks.join('\n'));
        },
      };
    },
  },
};
globalThis.context = {
  repo: { owner: 'owner', repo: 'repository' },
  payload: { pull_request: { number: 7 } },
};
globalThis.github = {
  rest: {
    issues: {
      getComment: async ({ comment_id }) => {
        fs.appendFileSync(process.env.GET_LOG, `${comment_id}\n`);
        const comment = JSON.parse(fs.readFileSync(process.env.COMMENT_FILE, 'utf8'));
        return { data: comment };
      },
      updateComment: async ({ comment_id, body }) => {
        fs.appendFileSync(process.env.UPDATE_LOG, `${comment_id}\n`);
        fs.writeFileSync(process.env.UPDATED_BODY, body);
      },
    },
  },
};
"#;

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn step_range<'a>(workflow: &'a str, step: &str) -> (Vec<&'a str>, usize, usize) {
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

    (lines, start, end)
}

fn step_text(workflow: &str, step: &str) -> String {
    let (lines, start, end) = step_range(workflow, step);

    format!("{}\n", lines[start..end].join("\n"))
}

fn step_block(workflow: &str, step: &str, key: &str) -> String {
    let (lines, start, end) = step_range(workflow, step);
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

fn remove_stale(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn last_output(output: &str, key: &str) -> Option<String> {
    let lines: Vec<&str> = output.lines().collect();
    let assignment = format!("{key}=");
    let heredoc = format!("{key}<<");
    let mut last = None;
    let mut start = 0;

    while start < lines.len() {
        let line = lines[start];
        let next = start + 1;

        if let Some(value) = line.strip_prefix(&assignment) {
            last = Some(value.to_string());
            start = next;
            continue;
        }

        let Some(delimiter) = heredoc_delimiter(line) else {
            start = next;
            continue;
        };
        let end = lines[next..]
            .iter()
            .position(|candidate| *candidate == delimiter)
            .map(|offset| next + offset);

        if line.starts_with(&heredoc) {
            last = end.map(|end| lines[next..end].join("\n"));
        }
        start = end.map_or(lines.len(), |end| end + 1);
    }

    last
}

fn heredoc_delimiter(line: &str) -> Option<&str> {
    let (name, delimiter) = line.split_once("<<")?;
    let valid_name = !name.is_empty()
        && !name.contains(|character: char| character.is_whitespace() || character == '=');
    let valid_delimiter = !delimiter.is_empty() && !delimiter.contains(char::is_whitespace);

    (valid_name && valid_delimiter).then_some(delimiter)
}

#[derive(Clone, Copy)]
struct Run<'a> {
    ocr_exit: Option<i32>,
    first: i32,
    result: Option<&'a str>,
    resume: i32,
    resumed: i32,
    resume_result: Option<&'a str>,
    resume_unusable: bool,
}

impl Default for Run<'_> {
    fn default() -> Self {
        Self {
            ocr_exit: Some(0),
            first: 0,
            result: None,
            resume: 0,
            resumed: 0,
            resume_result: None,
            resume_unusable: false,
        }
    }
}

struct Classifier {
    folder: TempDir,
}

struct Observation {
    exit: i32,
    stdout: String,
    output: String,
    coverage: Option<String>,
    ocr_arguments: Option<String>,
}

impl Observation {
    fn final_status(&self) -> Option<String> {
        last_output(&self.output, "final")
    }

    fn reason(&self) -> Option<String> {
        last_output(&self.output, "reason")
    }

    fn resumed(&self) -> bool {
        self.output.lines().any(|line| line == "resumed=true")
    }

    fn post_resumed(&self) -> bool {
        self.output.lines().any(|line| line == "post_resumed=true")
    }

    fn coverage_source(&self) -> Option<String> {
        last_output(&self.output, "coverage_source")
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

        let script = step_block(OCR_REVIEW, CLASSIFY_STEP, "run")
            .replace("/tmp/", &format!("{}/", results.display()));
        fs::write(root.join("classify.sh"), script).expect("write the extracted script");

        let report = step_block(OCR_REVIEW, REPORT_STEP, "run")
            .replace("/tmp/", &format!("{}/", results.display()));
        fs::write(root.join("report.sh"), report).expect("write the extracted report script");

        Classifier { folder }
    }

    fn run(&self, run: Run<'_>) -> Observation {
        let root = self.folder.path();
        let results = root.join("ocr");
        let result_file = results.join("ocr-result.json");
        let output_file = root.join("output");
        remove_stale(&result_file).expect("remove the stale result file");
        remove_stale(&output_file).expect("remove the stale output file");
        remove_stale(&results.join("ocr-coverage.md")).expect("remove the stale coverage file");
        remove_stale(&results.join("ocr-coverage.err")).expect("remove the stale coverage errors");
        remove_stale(&results.join("ocr-first-coverage.md"))
            .expect("remove the stale first coverage");
        remove_stale(&results.join("ocr-resume.json")).expect("remove the stale resume result");
        remove_stale(&results.join("ocr-resume-coverage.md"))
            .expect("remove the stale resume coverage");
        remove_stale(&results.join("ocr-resume-coverage.err"))
            .expect("remove the stale resume errors");
        remove_stale(&root.join("ocr-arguments")).expect("remove the stale arguments");

        if let Some(result) = run.result {
            fs::write(&result_file, result).expect("write the result file");
        }

        let mut command = Command::new("/bin/bash");
        command
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
            .env("STOMME_FIRST_CODE", run.first.to_string())
            .env("STOMME_RESUME_CODE", run.resumed.to_string())
            .env("OCR_RESUME_CODE", run.resume.to_string())
            .env("OCR_ARGUMENTS", root.join("ocr-arguments"));

        if let Some(ocr_exit) = run.ocr_exit {
            command.env("OCR_EXIT_CODE", ocr_exit.to_string());
        }

        if let Some(result) = run.resume_result {
            command.env("OCR_RESUME_BODY", result);
        }

        if run.resume_unusable {
            command.env("STOMME_RESUME_UNUSABLE", "1");
        }

        let completed = command
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
            ocr_arguments: read(&root.join("ocr-arguments")),
        }
    }

    fn report(&self, range_reason: &str, status: &str, reason: &str) -> (i32, String) {
        let root = self.folder.path();
        let completed = Command::new("/bin/bash")
            .arg("-e")
            .arg(root.join("report.sh"))
            .current_dir(root)
            .env_clear()
            .env("PATH", search_path(&root.join("bin")))
            .env("RANGE_REASON", range_reason)
            .env("FINAL", status)
            .env("REASON", reason)
            .output()
            .expect("run the extracted report script");

        (
            completed
                .status
                .code()
                .expect("the report script exits normally"),
            String::from_utf8_lossy(&completed.stdout).into_owned(),
        )
    }
}

#[test]
fn stale_file_removal_ignores_only_missing_files() {
    let folder = temporary_directory();
    remove_stale(&folder.path().join("missing")).expect("a missing file is ignored");

    let directory = folder.path().join("directory");
    fs::create_dir(&directory).expect("create a directory");
    let error = remove_stale(&directory).expect_err("a directory is not a stale file");

    assert_ne!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn workflow_classifies_with_the_released_binary() {
    assert!(!OCR_REVIEW.contains("build-stomme"));
    assert!(!OCR_REVIEW.contains("Build stomme from the change"));
    assert!(!OCR_REVIEW.contains("unset MODEL_TOKEN"));
    assert!(!OCR_CALLER.contains("with:"));

    let install = step_text(OCR_REVIEW, "Install stomme from the project pin");

    assert!(install.contains("uses: jdx/mise-action@"));
    assert!(!install.contains("\n        with:\n"));
}

#[test]
fn workflow_resumes_only_a_budget_partial() {
    let classify = step_text(OCR_REVIEW, CLASSIFY_STEP);

    assert!(classify.contains("if: steps.ocr.outputs.range_reason != 'same_head_noop'"));
    assert!(classify.contains("if [ \"${OCR_EXIT_CODE:-}\" != \"0\" ]; then"));
    assert!(classify.contains("if [ \"$final\" != \"2\" ]; then"));
    assert!(classify.contains("session_id=\"$(jq -r '.session_id // empty'"));
    assert!(!classify.contains("build-stomme"));
}

#[test]
fn workflow_posts_and_reports_the_resumed_result() {
    let post = step_text(OCR_REVIEW, "Post the resumed review");

    assert!(post.contains("id: resume-post"));
    assert!(post.contains("if: steps.outcome.outputs.post_resumed == 'true'"));

    let coverage = step_text(OCR_REVIEW, COVERAGE_STEP);

    assert!(coverage.contains("steps.ocr.outputs.summary_comment_url"));
    assert!(coverage.contains("steps.resume-post.outputs.summary_comment_url"));
    assert!(coverage.contains("steps.outcome.outputs.coverage_source"));
    assert!(coverage.contains("#issuecomment-"));
    assert!(!coverage.contains("findSummaryIssueComment"));
}

#[test]
fn workflow_routes_the_job_from_its_predicates() {
    assert!(OCR_REVIEW.contains("if: ${{ !github.event.pull_request.draft }}"));

    let token = step_text(OCR_REVIEW, "Check the model token");

    assert!(token.contains("if [ -z \"$MODEL_TOKEN\" ]"));

    let coverage = step_text(OCR_REVIEW, COVERAGE_STEP);

    assert!(
        coverage.contains("if: always() && steps.ocr.outputs.range_reason != 'same_head_noop'")
    );

    let report = step_block(OCR_REVIEW, REPORT_STEP, "run");

    assert!(report.contains("if [ \"$RANGE_REASON\" = \"same_head_noop\" ]"));
}

#[test]
fn report_decides_the_check_result() {
    let classifier = Classifier::new();

    let (exit, stdout) = classifier.report("ok", "0", "");

    assert_eq!(exit, 0);
    assert!(stdout.contains("covered every selected file"), "{stdout}");

    let (exit, stdout) = classifier.report("ok", "2", "");

    assert_eq!(exit, 1);
    assert!(
        stdout.contains("::error::the OCR review left selected files unreviewed"),
        "{stdout}"
    );

    let (exit, stdout) = classifier.report("ok", "1", "provider");

    assert_eq!(exit, 1);
    assert!(
        stdout.contains("::error::the OCR review failed: provider"),
        "{stdout}"
    );

    let (exit, stdout) = classifier.report("ok", "1", "");

    assert_eq!(exit, 1);
    assert!(
        stdout.contains("::error::the OCR review failed\n"),
        "{stdout}"
    );

    let (exit, _) = classifier.report("same_head_noop", "", "");

    assert_eq!(exit, 0);
}

#[test]
fn classifier_passes_a_complete_first_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        result: Some(SESSION_RESULT),
        ..Run::default()
    });

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
    let observation = classifier.run(Run {
        first: 1,
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(!observation.resumed());
    assert_eq!(observation.coverage, None);
    assert_eq!(observation.ocr_arguments, None);
}

#[test]
fn classifier_warns_and_fails_a_partial_without_a_session() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(EMPTY_RESULT),
        ..Run::default()
    });

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
fn classifier_does_not_resume_a_provider_failure() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 2,
        result: Some(MIXED_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(!observation.resumed());
    assert_eq!(
        observation.ocr_arguments, None,
        "a provider failure is never resumed"
    );
    assert_eq!(observation.reason().as_deref(), Some("provider"));
    assert_eq!(observation.coverage_source().as_deref(), Some("first"));
    assert!(
        observation
            .coverage
            .expect("the mixed result writes coverage")
            .contains("ocr-result.json")
    );

    let (exit, stdout) = classifier.report("ok", "1", "provider");

    assert_eq!(exit, 1);
    assert!(
        stdout.contains("::error::the OCR review failed: provider"),
        "{stdout}"
    );
    assert!(stdout.contains("not reviewed"), "{stdout}");
}

#[test]
fn classifier_does_not_resume_without_an_ocr_exit_code() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: None,
        first: 2,
        result: Some(SESSION_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(!observation.resumed());
    assert_eq!(observation.ocr_arguments, None);
}

#[test]
fn classifier_takes_the_run_failure_reason_first() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 1,
        result: Some(TIMEOUT_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(
        observation.reason().as_deref(),
        Some("the global deadline elapsed")
    );
    assert_eq!(observation.ocr_arguments, None);
}

#[test]
fn classifier_keeps_a_reason_that_spans_lines() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 1,
        result: Some(MULTILINE_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(
        observation.reason().as_deref(),
        Some("first line\nsecond line")
    );
}

#[test]
fn classifier_ignores_assignments_inside_a_reason_body() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 1,
        result: Some(ASSIGNMENT_REASON_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(observation.coverage_source().as_deref(), Some("first"));
    assert_eq!(
        observation.reason().as_deref(),
        Some("first line\nfinal=0\ncoverage_source=resumed")
    );
}

#[test]
fn classifier_uses_the_complete_resumed_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("0"));
    assert!(observation.resumed());
    assert!(observation.post_resumed());
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
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resumed: 2,
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("2"));
    assert!(observation.resumed());
    assert!(observation.post_resumed());
}

#[test]
fn classifier_reports_a_failed_resumed_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resumed: 1,
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(observation.resumed());
    assert!(!observation.post_resumed());
    assert!(observation.reason().is_some());
}

#[test]
fn classifier_posts_a_partial_resume_that_exits_nonzero() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resume: 1,
        resumed: 2,
        resume_result: Some(MIXED_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(observation.reason().as_deref(), Some("provider"));
    assert!(observation.resumed());
    assert_eq!(observation.coverage_source().as_deref(), Some("resumed"));
    assert!(
        observation.post_resumed(),
        "a usable partial result is posted even when the resume exits nonzero"
    );
    assert!(
        observation
            .coverage
            .as_deref()
            .expect("the resumed result keeps its coverage")
            .contains("ocr-resume.json")
    );

    let (exit, stdout) = classifier.report("ok", "1", "provider");

    assert_eq!(exit, 1);
    assert!(stdout.contains("not reviewed"), "{stdout}");
}

#[test]
fn classifier_keeps_the_first_coverage_when_the_resume_is_unusable() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resume_unusable: true,
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert!(!observation.post_resumed());
    assert_eq!(observation.coverage_source().as_deref(), Some("first"));
    assert!(
        observation
            .coverage
            .as_deref()
            .expect("the first result keeps its coverage")
            .contains("ocr-result.json")
    );
    assert!(
        observation
            .ocr_arguments
            .as_deref()
            .expect("the resume ran")
            .contains("--resume session-1")
    );
    assert_eq!(
        observation.reason().as_deref(),
        Some("the OCR run recorded no failure reason")
    );
}

struct Coverage {
    folder: TempDir,
    workspace: PathBuf,
}

struct CoverageObservation {
    exit: i32,
    updated: Option<String>,
    get: Option<String>,
    update: Option<String>,
    info: String,
    warnings: String,
    summary: Option<String>,
}

#[derive(Clone, Copy, Default)]
struct CoverageRun<'a> {
    coverage: Option<&'a str>,
    comment: Option<&'a str>,
    first_url: Option<&'a str>,
    resumed_url: Option<&'a str>,
    from_resume: bool,
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

    fn run(&self, run: CoverageRun<'_>) -> CoverageObservation {
        let root = self.folder.path();
        let coverage_file = root.join("ocr-coverage.md");
        let comment_file = root.join("comment.json");
        let updated_file = root.join("updated.txt");
        let get_file = root.join("get.txt");
        let update_file = root.join("update.txt");
        let info_file = root.join("info.txt");
        let warning_file = root.join("warning.txt");
        let summary_file = root.join("summary.txt");
        for path in [
            &coverage_file,
            &comment_file,
            &updated_file,
            &get_file,
            &update_file,
            &info_file,
            &warning_file,
            &summary_file,
        ] {
            remove_stale(path).expect("remove the stale coverage output");
        }

        if let Some(coverage) = run.coverage {
            fs::write(&coverage_file, coverage).expect("write the coverage file");
        }

        if let Some(body) = run.comment {
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
            .env("GET_LOG", &get_file)
            .env("UPDATE_LOG", &update_file)
            .env("INFO_LOG", &info_file)
            .env("WARNING_LOG", &warning_file)
            .env("SUMMARY_LOG", &summary_file)
            .env("FIRST_COMMENT_URL", run.first_url.unwrap_or_default())
            .env("RESUMED_COMMENT_URL", run.resumed_url.unwrap_or_default())
            .env(
                "COVERAGE_SOURCE",
                if run.from_resume { "resumed" } else { "first" },
            )
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
            get: read(&get_file),
            update: read(&update_file),
            info: read(&info_file).unwrap_or_default(),
            warnings: read(&warning_file).unwrap_or_default(),
            summary: read(&summary_file),
        }
    }
}

#[test]
fn coverage_inserts_the_block_after_the_summary_marker() {
    let coverage = Coverage::new();
    let observation = coverage.run(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some("<!-- ocr-summary -->\nSummary text"),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
        ..CoverageRun::default()
    });

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
    let observation = coverage.run(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some("<!-- ocr-summary -->\n<!-- ocr-summary-run: 123 -->\nSummary text"),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
        ..CoverageRun::default()
    });

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
    let observation = coverage.run(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some(
            "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\nold\n<!-- ocr-coverage:end -->\nSummary",
        ),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
        ..CoverageRun::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(
        observation.updated.as_deref(),
        Some(
            "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\nCovered: src/lib.rs\n<!-- ocr-coverage:end -->\nSummary"
        )
    );
}

#[test]
fn coverage_edits_the_comment_this_run_posted() {
    let coverage = Coverage::new();
    let observation = coverage.run(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some("<!-- ocr-summary -->\nSummary text"),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
        ..CoverageRun::default()
    });

    assert_eq!(observation.get.as_deref(), Some("42\n"));
    assert_eq!(observation.update.as_deref(), Some("42\n"));
    assert!(
        observation
            .updated
            .expect("the posted comment is updated")
            .contains("Covered: src/lib.rs")
    );
}

#[test]
fn coverage_uses_the_resumed_post_url() {
    let coverage = Coverage::new();
    let observation = coverage.run(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some("<!-- ocr-summary -->\nSummary text"),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-41"),
        resumed_url: Some("https://github.com/owner/repository/pull/7#issuecomment-43"),
        from_resume: true,
    });

    assert_eq!(observation.get.as_deref(), Some("43\n"));
    assert_eq!(observation.update.as_deref(), Some("43\n"));
}

#[test]
fn coverage_ignores_the_first_comment_for_an_unposted_resume() {
    let coverage = Coverage::new();
    let observation = coverage.run(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some("<!-- ocr-summary -->\nSummary text"),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-41"),
        from_resume: true,
        ..CoverageRun::default()
    });

    assert_eq!(observation.get, None);
    assert_eq!(observation.update, None);
    assert!(
        observation
            .summary
            .expect("the coverage goes to the job summary")
            .contains("Covered: src/lib.rs")
    );
}

#[test]
fn coverage_leaves_the_comment_alone_without_a_url() {
    let coverage = Coverage::new();
    let observation = coverage.run(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some("<!-- ocr-summary -->\nSummary text"),
        ..CoverageRun::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.updated, None);
    assert_eq!(observation.get, None);
    assert_eq!(observation.update, None);
    assert!(
        observation
            .summary
            .expect("the coverage goes to the job summary")
            .contains("Covered: src/lib.rs")
    );
    assert!(observation.info.contains(
        "This run posted no summary comment; the coverage section is only in the job summary."
    ));
}

#[test]
fn coverage_leaves_the_comment_alone_without_coverage() {
    let coverage = Coverage::new();
    let comment = Some("<!-- ocr-summary -->\nSummary text");
    let url = Some("https://github.com/owner/repository/pull/7#issuecomment-42");
    let missing = coverage.run(CoverageRun {
        comment,
        first_url: url,
        ..CoverageRun::default()
    });

    assert_eq!(missing.updated, None);
    assert_eq!(missing.get, None);
    assert!(
        missing
            .info
            .contains("No coverage file; leaving the summary comment alone.")
    );

    let empty = coverage.run(CoverageRun {
        coverage: Some("  \n"),
        comment,
        first_url: url,
        ..CoverageRun::default()
    });

    assert_eq!(empty.updated, None);
    assert_eq!(empty.get, None);
    assert!(
        empty
            .info
            .contains("The coverage file is empty; leaving the summary comment alone.")
    );
}

#[test]
fn coverage_warns_when_the_summary_has_no_marker() {
    let coverage = Coverage::new();
    let observation = coverage.run(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some("No marker here"),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
        ..CoverageRun::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.updated, None);
    assert!(
        observation
            .warnings
            .contains("The summary comment has no marker; not inserting the coverage section.")
    );
}
