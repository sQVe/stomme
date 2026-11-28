use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

use crate::test_support::{temporary_directory, write_executable};

const OCR_REVIEW: &str = include_str!("../../.github/workflows/ocr-review.yml");
const OCR_CALLER: &str = include_str!("../../.github/workflows/ocr.yml");
const POSTING_HELPER: &str = include_str!("../fixtures/ocr-post-review-comments-exports.js");
const REFS_STEP: &str = "Resolve the review refs";
const RANGE_STEP: &str = "Resolve the review range";
const SCOPE_STEP: &str = "Build the review scope";
const REVIEW_STEP: &str = "Run the review";
const CLASSIFY_STEP: &str = "Classify and resume the review";
const POST_STEP: &str = "Post the review";
const RESUME_POST_STEP: &str = "Post the resumed review";
const COVERAGE_STEP: &str = "Add the coverage section";
const CHECKPOINT_STEP: &str = "Read the checkpointed coverage";
const REPORT_STEP: &str = "Report the OCR result";
const HEAD_SHA: &str = "8c6c9a2f49891c7f86c7a82f380f5c32519c52a6";
const OTHER_HEAD: &str = "0b836765f754d4d652906821e5d850a9c90d778f";
const SCOPED_BASE: &str = "1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d";
const MERGE_BASE: &str = "aabbccddeeff00112233445566778899aabbccdd";
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
const MIXED_CAUSE_RESULT: &str = r#"{
  "status": "partial",
  "session_id": "session-4",
  "manifest": {
    "schema_version": "ocr.run-manifest/v1",
    "terminal_state": "partial",
    "coverage": {
      "selected": [
        {"item_id": "a", "path": "src/a.rs"},
        {"item_id": "b", "path": "src/b.rs"}
      ],
      "completed": [],
      "reused": [],
      "failed": [
        {"item_id": "a", "path": "src/a.rs", "classification": "provider", "reason": "upstream 500"},
        {"item_id": "b", "path": "src/b.rs", "classification": "timeout", "reason": "the item deadline elapsed"}
      ],
      "waived": [],
      "excluded": []
    }
  }
}"#;

const WAIVED_RESULT: &str = r#"{
  "status": "partial",
  "session_id": "session-5",
  "manifest": {
    "schema_version": "ocr.run-manifest/v1",
    "terminal_state": "partial",
    "coverage": {
      "selected": [
        {"item_id": "a", "path": "src/a.rs"},
        {"item_id": "b", "path": "src/b.rs"},
        {"item_id": "c", "path": "src/c.rs"}
      ],
      "completed": [{"item_id": "a", "path": "src/a.rs"}],
      "reused": [],
      "failed": [
        {"item_id": "b", "path": "src/b.rs", "classification": "provider", "reason": "upstream 500"}
      ],
      "waived": [{"item_id": "c", "path": "src/c.rs", "reason": "the file is too large"}],
      "excluded": []
    }
  }
}"#;

const FAILED_PROVIDER_RESULT: &str = r#"{
  "status": "failed",
  "session_id": "session-3",
  "manifest": {
    "schema_version": "ocr.run-manifest/v1",
    "terminal_state": "failed",
    "coverage": {
      "selected": [{"item_id": "a", "path": "src/a.rs"}],
      "completed": [],
      "reused": [],
      "failed": [{"item_id": "a", "path": "src/a.rs", "classification": "provider", "reason": "upstream 500"}],
      "waived": [],
      "excluded": []
    }
  }
}"#;

const BUDGET_RESULT: &str = r#"{
  "status": "partial",
  "session_id": "session-1",
  "manifest": {
    "schema_version": "ocr.run-manifest/v1",
    "terminal_state": "partial",
    "coverage": {
      "selected": [{"item_id": "a", "path": "src/a.rs"}],
      "completed": [],
      "reused": [],
      "failed": [{"item_id": "a", "path": "src/a.rs", "classification": "budget", "reason": "the token budget ran out"}],
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
  *ocr-result.json)
    if [ "${STOMME_FIRST_UNUSABLE:-}" = "1" ]; then
      echo "stomme: $2 is not a usable OCR result" >&2
      exit 1
    fi
    ;;
esac
echo "coverage from $2"
if [ -n "${STOMME_STDERR_LINE:-}" ]; then
  echo "$STOMME_STDERR_LINE" >&2
else
  echo "stomme: $2: not reviewed (demo)" >&2
fi
case "$2" in
  *ocr-result.json) exit "${STOMME_FIRST_CODE:-0}" ;;
  *ocr-resume.json) exit "${STOMME_RESUME_CODE:-0}" ;;
esac
exit 0
"#;

const OCR_FAKE: &str = r#"#!/bin/bash
printf '%s\n' "$*" >> "$OCR_ARGUMENTS"
if [ -n "${OCR_STDERR:-}" ]; then
  echo "$OCR_STDERR" >&2
fi
if [ "${OCR_SILENT:-}" = "1" ]; then
  echo "ocr: the provider request failed" >&2
  exit "${OCR_RESUME_CODE:-1}"
fi
body="${OCR_RESUME_BODY-}"
if [ -z "$body" ]; then
  body='{"session_id":"session-1"}'
fi
printf '%s\n' "$body"
exit "${OCR_RESUME_CODE:-0}"
"#;

const RANGE_HELPER: &str = r#"const fs = require('fs');
module.exports = {
  ...require(process.env.PINNED_HELPER),
  readCheckpointComment: async (options) => {
    fs.writeFileSync(process.env.LOOKUP_LOG, JSON.stringify({
      owner: options.owner,
      repo: options.repo,
      prNumber: options.prNumber,
      appSlug: options.appSlug,
      log: typeof options.log,
      github: typeof options.github,
    }));
    return JSON.parse(process.env.CHECKPOINT_RESULT);
  },
  // The pinned helper's own validation, so the workflow is judged by the rule
  // that ships with the action it pins.
  validateCheckpointPayload: (payload, { prNumber } = {}) => {
    if (!payload) return 'no payload';
    if (payload.v !== 1) return 'unsupported version';
    if (prNumber != null && payload.pr !== prNumber) return 'belongs to another pull request';
    if (!/^[0-9a-f]{40}$/.test(payload.head)) return 'head is not a 40-hex sha';
    if (payload.terminal_state !== 'complete') return 'terminal_state is not complete';
    if (typeof payload.base_ref !== 'string' || payload.base_ref === '') return 'missing base_ref';
    if (typeof payload.merge_base !== 'string' || payload.merge_base === '') return 'missing merge_base';
    if (typeof payload.fingerprint !== 'string' || payload.fingerprint === '') return 'missing fingerprint';
    return null;
  },
};
"#;

const GIT_FAKE: &str = r#"#!/bin/bash
printf '%s\n' "$*" >> "$GIT_ARGUMENTS"
if [ "${GIT_FAIL:-}" = "1" ]; then
  echo "fatal: could not read from remote repository" >&2
  exit 128
fi
exit 0
"#;

const SCOPE_FAKE: &str = r#"#!/bin/bash
printf '%s\n' "$*" >> "$SCOPE_ARGUMENTS"
if [ -n "${SCOPE_STDERR:-}" ]; then
  printf '%s\n' "$SCOPE_STDERR" >&2
fi
if [ "${SCOPE_EXIT:-0}" != "0" ]; then
  exit "$SCOPE_EXIT"
fi
printf '%s\n' "${SCOPE_BASE-}"
"#;

const REVIEW_FAKE: &str = r#"#!/bin/bash
printf '%s\n' "$*" >> "$OCR_ARGUMENTS"
body="${OCR_REVIEW_BODY-}"
if [ -z "$body" ]; then
  body='{"session_id":"session-1"}'
fi
printf '%s\n' "$body"
exit "${OCR_REVIEW_EXIT:-0}"
"#;

const EXPORTS_PROBE: &str = r#"const helper = require(process.env.PINNED_HELPER);
process.stdout.write(JSON.stringify({
  marker: helper.SUMMARY_MARKER,
  exports: Object.keys(helper).sort(),
}));
"#;

const POSTING_HELPER_RECORD: &str = r#"const fs = require('fs');
const injected = ['github', 'context', 'core', 'fs'];
module.exports = {
  ...require(process.env.PINNED_HELPER),
  runPostReviewComments: async (options) => {
    const recorded = {};
    for (const [key, value] of Object.entries(options)) {
      recorded[key] = injected.includes(key) ? typeof value : value;
    }
    fs.writeFileSync(process.env.RECORD_LOG, JSON.stringify(recorded));
  },
};
"#;

const CHECKPOINT_HELPER: &str = r#"const fs = require('fs');
module.exports = {
  ...require(process.env.PINNED_HELPER),
  findSummaryIssueComment: async (options) => {
    const recorded = {
      owner: options.owner,
      repo: options.repo,
      prNumber: options.prNumber,
      sticky: options.sticky,
      appSlug: options.appSlug,
      log: typeof options.log,
      github: typeof options.github,
    };
    fs.writeFileSync(process.env.LOOKUP_LOG, JSON.stringify(recorded));
    if (!fs.existsSync(process.env.COMMENT_FILE)) return null;
    return JSON.parse(fs.readFileSync(process.env.COMMENT_FILE, 'utf8'));
  },
};
"#;

const RUNNER_PREFIX: &str = r#"const fs = require('fs');
const chunks = [];
globalThis.core = {
  info: (line) => fs.appendFileSync(process.env.INFO_LOG, `${line}\n`),
  warning: (line) => fs.appendFileSync(process.env.WARNING_LOG, `${line}\n`),
  setOutput: (name, value) => fs.appendFileSync(process.env.GITHUB_OUTPUT, `${name}=${value}\n`),
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

fn step_guard(step: &str) -> &str {
    step.lines()
        .find_map(|line| line.trim().strip_prefix("if: "))
        .expect("the step has a guard")
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

fn search_path(folder: &Path) -> String {
    let inherited = std::env::var("PATH").unwrap_or_default();

    format!("{}:{inherited}", folder.display())
}

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn posting_helper_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/ocr-post-review-comments-exports.js")
}

fn verdict_line(head: &str, verdict: &str) -> String {
    format!("<!-- stomme-ocr-coverage-verdict: {verdict} head={head} -->")
}

fn write_posting_helper(workspace: &Path, text: &str) {
    let helper = workspace
        .join(".ocr-action")
        .join("scripts")
        .join("github-actions")
        .join("post-review-comments.js");
    fs::create_dir_all(helper.parent().expect("the helper has a folder"))
        .expect("create the helper folder");
    fs::write(&helper, text).expect("write the helper");
}

fn github_script_runner(script: &str) -> String {
    format!(
        "{RUNNER_PREFIX}\n(async () => {{\n{script}}})().catch((error) => {{\n  console.error(error);\n  process.exit(1);\n}});\n"
    )
}

fn write_github_script(folder: &Path, step: &str) {
    let script = step_block(OCR_REVIEW, step, "script");
    fs::write(folder.join("runner.js"), github_script_runner(&script)).expect("write the runner");
}

fn run_github_script(folder: &Path, environment: &[(&str, String)]) -> Output {
    let mut command = Command::new("node");
    command
        .arg(folder.join("runner.js"))
        .current_dir(folder)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default());

    for (key, value) in environment {
        command.env(key, value);
    }

    command
        .output()
        .expect("run the extracted script with node")
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
struct Report<'a> {
    range_mode: &'a str,
    scope_mode: &'a str,
    status: &'a str,
    reason: &'a str,
    verdict: &'a str,
}

impl Default for Report<'_> {
    fn default() -> Self {
        Self {
            range_mode: "full",
            scope_mode: "",
            status: "0",
            reason: "",
            verdict: "",
        }
    }
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
    resume_silent: bool,
    resume_stderr: Option<&'a str>,
    first_unusable: bool,
    stderr_line: Option<&'a str>,
    range_from: &'a str,
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
            resume_silent: false,
            resume_stderr: None,
            first_unusable: false,
            stderr_line: None,
            range_from: "range-from",
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
    resume_errors: Option<String>,
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

    fn resume_errors(&self) -> Option<&str> {
        self.resume_errors.as_deref()
    }

    fn resume_calls(&self) -> usize {
        self.ocr_arguments
            .as_deref()
            .map_or(0, |arguments| arguments.lines().count())
    }

    fn coverage_source(&self) -> Option<String> {
        last_output(&self.output, "coverage_source")
    }

    fn coverage_verdict(&self) -> Option<String> {
        last_output(&self.output, "coverage_verdict")
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
        remove_stale(&results.join("ocr-resume.err")).expect("remove the stale retry errors");
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
            .env("RANGE_FROM", run.range_from)
            .env("OCR_RULE", "docs/stomme/ocr-rules.json")
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

        if run.first_unusable {
            command.env("STOMME_FIRST_UNUSABLE", "1");
        }

        if run.resume_silent {
            command.env("OCR_SILENT", "1");
        }

        if let Some(line) = run.resume_stderr {
            command.env("OCR_STDERR", line);
        }

        if let Some(line) = run.stderr_line {
            command.env("STOMME_STDERR_LINE", line);
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
            resume_errors: read(&results.join("ocr-resume.err")),
            ocr_arguments: read(&root.join("ocr-arguments")),
        }
    }

    fn report(&self, report: Report<'_>) -> (i32, String) {
        let root = self.folder.path();
        let completed = Command::new("/bin/bash")
            .arg("-e")
            .arg(root.join("report.sh"))
            .current_dir(root)
            .env_clear()
            .env("PATH", search_path(&root.join("bin")))
            .env("RANGE_MODE", report.range_mode)
            .env("SCOPE_MODE", report.scope_mode)
            .env("FINAL", report.status)
            .env("REASON", report.reason)
            .env("CHECKPOINT_VERDICT", report.verdict)
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
fn workflow_runs_the_review_itself() {
    assert!(
        !OCR_REVIEW.contains("uses: alibaba/open-code-review@"),
        "the workflow runs the review instead of the action's review step"
    );

    let review = step_block(OCR_REVIEW, REVIEW_STEP, "run");

    assert!(review.contains("ocr review \"${args[@]}\""), "{review}");
    assert!(review.contains("--rule \"$OCR_RULE\""), "{review}");
    assert!(review.contains("--format json"), "{review}");
    assert!(
        OCR_REVIEW.contains("RESOLVE_OUTDATED: 'true'"),
        "the posting helper resolves outdated threads, as the action's input did"
    );
}

#[test]
fn workflow_installs_and_configures_the_pinned_reviewer() {
    let install = step_block(OCR_REVIEW, "Install the pinned OpenCodeReview", "run");

    assert!(
        install.contains("@alibaba-group/open-code-review@${OCR_VERSION}"),
        "{install}"
    );
    assert!(OCR_REVIEW.contains("OCR_VERSION: '1.14.2'"));

    let configure = step_block(OCR_REVIEW, "Configure OCR", "run");

    for line in [
        "ocr config set llm.url \"$OCR_LLM_URL\"",
        "ocr config set llm.model \"$OCR_LLM_MODEL\"",
        "ocr config set llm.protocol \"openai\"",
        "ocr config set language \"English\"",
        "ocr config set llm.extra_body \"$OCR_EXTRA_BODY\"",
    ] {
        assert!(configure.contains(line), "{line}\n{configure}");
    }
}

#[test]
fn workflow_skips_the_install_for_a_no_op_review() {
    let install = step_text(OCR_REVIEW, "Install stomme from the project pin");
    let classify = step_text(OCR_REVIEW, CLASSIFY_STEP);

    assert_eq!(
        step_guard(&install),
        "steps.range.outputs.mode != 'noop'",
        "a no-op review needs no binary"
    );
    assert_eq!(step_guard(&classify), "steps.range.outputs.mode != 'noop'");
}

#[test]
fn workflow_guards_each_route_from_the_range_decision() {
    let scope = step_text(OCR_REVIEW, SCOPE_STEP);
    let review = step_text(OCR_REVIEW, REVIEW_STEP);
    let post = step_text(OCR_REVIEW, POST_STEP);
    let coverage = step_text(OCR_REVIEW, COVERAGE_STEP);
    let read = step_text(OCR_REVIEW, CHECKPOINT_STEP);

    assert_eq!(step_guard(&scope), "steps.range.outputs.mode == 'scoped'");
    assert_eq!(
        step_guard(&review),
        "steps.range.outputs.mode != 'noop' && steps.scope.outputs.mode != 'empty'",
        "an empty scope runs no review"
    );
    assert_eq!(
        step_guard(&post),
        "steps.review.outputs.exit_code == '0' || steps.scope.outputs.mode == 'empty'",
        "an empty scope posts the result that advances the marker"
    );
    assert_eq!(
        step_guard(&coverage),
        "always() && steps.range.outputs.mode != 'noop'"
    );
    assert_eq!(
        step_guard(&read),
        "always() && steps.range.outputs.mode == 'noop'"
    );
}

#[test]
fn workflow_classifies_the_review_of_every_run() {
    let classify = step_text(OCR_REVIEW, CLASSIFY_STEP);

    assert!(classify.contains("steps.range.outputs.mode != 'noop'"));
    assert!(classify.contains("RANGE_FROM: ${{ steps.scope.outputs.base }}"));
    assert!(
        classify.contains("echo \"first_final=$final\""),
        "the first result's own verdict decides whether the marker may advance"
    );
    assert!(!classify.contains("build-stomme"));
}

#[test]
fn workflow_posts_and_reports_the_resumed_result() {
    let post = step_text(OCR_REVIEW, RESUME_POST_STEP);

    assert!(post.contains("id: resume-post"));
    assert!(post.contains("if: steps.outcome.outputs.post_resumed == 'true'"));

    let coverage = step_text(OCR_REVIEW, COVERAGE_STEP);

    assert!(coverage.contains("steps.post.outputs.summary_comment_url"));
    assert!(coverage.contains("steps.resume-post.outputs.summary_comment_url"));
    assert!(coverage.contains("steps.outcome.outputs.coverage_source"));
    assert!(coverage.contains("#issuecomment-"));
    assert!(coverage.contains("steps.scope.outputs.note"));
    assert!(!coverage.contains("findSummaryIssueComment"));
}

#[test]
fn workflow_routes_the_job_from_its_predicates() {
    assert!(OCR_REVIEW.contains("if: ${{ !github.event.pull_request.draft }}"));

    let token = step_text(OCR_REVIEW, "Check the model token");

    assert!(token.contains("if [ -z \"$MODEL_TOKEN\" ]"));

    let report = step_block(OCR_REVIEW, REPORT_STEP, "run");

    assert!(report.contains("if [ \"$RANGE_MODE\" = \"noop\" ]"));
    assert!(report.contains("if [ \"$SCOPE_MODE\" = \"empty\" ]"));
}

#[test]
fn workflow_resolves_the_refs_the_range_needs() {
    let refs = step_block(OCR_REVIEW, REFS_STEP, "run");

    assert!(refs.contains("git fetch origin \"pull/${PR_NUMBER}/head\""));
    assert!(refs.contains("git merge-base \"origin/${BASE_REF}\" \"$HEAD_SHA\""));
    assert!(
        refs.contains("^[0-9a-f]{40}$"),
        "the head SHA is validated before it reaches a step output"
    );

    let checkout = step_text(OCR_REVIEW, "Check out the pull request");

    assert!(checkout.contains("fetch-depth: 0"));
}

#[test]
fn the_pinned_posting_helper_exports_what_the_workflow_reads() {
    let completed = Command::new("node")
        .arg("-e")
        .arg(EXPORTS_PROBE)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("PINNED_HELPER", posting_helper_path())
        .output()
        .expect("run node");

    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );

    let printed: serde_json::Value =
        serde_json::from_slice(&completed.stdout).expect("node prints JSON");

    assert_eq!(printed["marker"], "<!-- ocr-summary -->");
    assert_eq!(
        printed["exports"],
        serde_json::json!([
            "SUMMARY_MARKER",
            "findSummaryIssueComment",
            "readCheckpointComment",
            "runPostReviewComments",
            "validateCheckpointPayload"
        ])
    );
}

#[test]
fn the_pinned_posting_helper_copy_names_the_commit_the_workflow_checks_out() {
    let commit = POSTING_HELPER
        .lines()
        .find_map(|line| line.strip_prefix("// commit: "))
        .expect("the pinned copy names its commit");

    assert!(
        OCR_REVIEW.contains(&format!("ref: {commit}")),
        "the workflow checks the posting helper out at {commit}"
    );
}

#[test]
fn workflow_reads_the_checkpointed_coverage_of_a_no_op_run() {
    let read = step_text(OCR_REVIEW, CHECKPOINT_STEP);

    assert!(read.contains("if: always() && steps.range.outputs.mode == 'noop'"));
    assert!(read.contains("helper.findSummaryIssueComment"));
    assert!(read.contains("core.setOutput('verdict'"));

    let checkout = step_text(OCR_REVIEW, "Check out the OCR posting helper");
    let order = |step: &str| {
        OCR_REVIEW
            .lines()
            .position(|line| line == format!("      - name: {step}"))
            .expect("the workflow has the step")
    };

    assert!(
        !checkout
            .lines()
            .any(|line| line.trim_start().starts_with("if: ")),
        "the helper is checked out for every route"
    );
    assert!(
        order("Check out the OCR posting helper") < order(RANGE_STEP),
        "the range decision reads the marker with the pinned helper"
    );
    assert!(
        order(RANGE_STEP) < order(CHECKPOINT_STEP),
        "the no-op route is chosen before its coverage is read"
    );

    let report = step_text(OCR_REVIEW, REPORT_STEP);

    assert!(report.contains("steps.checkpoint-coverage.outputs.verdict"));
    assert!(report.contains("cannot review the same head again"));
}

#[test]
fn workflow_passes_the_coverage_verdict_from_the_classifier_to_the_insert() {
    let classify = step_text(OCR_REVIEW, CLASSIFY_STEP);
    let insert = step_text(OCR_REVIEW, COVERAGE_STEP);

    for source in ["$final", "$resumed"] {
        let line =
            format!("echo \"coverage_verdict=$(verdict \"{source}\")\" >> \"$GITHUB_OUTPUT\"");

        assert!(
            classify.lines().any(|candidate| candidate.trim() == line),
            "the classifier records the {source} verdict:\n{classify}"
        );
    }

    let mapping = "COVERAGE_VERDICT: ${{ steps.outcome.outputs.coverage_verdict }}";

    assert!(
        insert.lines().any(|candidate| candidate.trim() == mapping),
        "the insert reads the recorded verdict:\n{insert}"
    );
}

#[test]
fn report_decides_the_check_result() {
    struct Case<'a> {
        name: &'a str,
        report: Report<'a>,
        expected_exit: i32,
        expected_fragments: &'a [&'a str],
        expected_line_count: Option<usize>,
    }

    let classifier = Classifier::new();
    let cases = [
        Case {
            name: "a complete run",
            report: Report {
                status: "0",
                ..Report::default()
            },
            expected_exit: 0,
            expected_fragments: &["covered every selected file"],
            expected_line_count: Some(1),
        },
        Case {
            name: "an unreviewed run",
            report: Report {
                status: "2",
                ..Report::default()
            },
            expected_exit: 1,
            expected_fragments: &["::error::the OCR review left selected files unreviewed"],
            expected_line_count: Some(1),
        },
        Case {
            name: "a failed run with a reason",
            report: Report {
                status: "1",
                reason: "provider",
                ..Report::default()
            },
            expected_exit: 1,
            expected_fragments: &["::error::the OCR review failed: provider"],
            expected_line_count: Some(1),
        },
        Case {
            name: "a failed run with encoded text in its reason",
            report: Report {
                status: "1",
                reason: "provider 50%0A",
                ..Report::default()
            },
            expected_exit: 1,
            expected_fragments: &["::error::the OCR review failed: provider 50%250A"],
            expected_line_count: Some(1),
        },
        Case {
            name: "a failed run with a multi-line reason",
            report: Report {
                status: "1",
                reason: "provider 50%0A\r\n::warning::forged",
                ..Report::default()
            },
            expected_exit: 1,
            expected_fragments: &[
                "::error::the OCR review failed: provider 50%250A%0D%0A::warning::forged",
            ],
            expected_line_count: Some(1),
        },
        Case {
            name: "a failed run without a reason",
            report: Report {
                status: "1",
                ..Report::default()
            },
            expected_exit: 1,
            expected_fragments: &["::error::the OCR review failed\n"],
            expected_line_count: Some(1),
        },
    ];

    for case in cases {
        let (exit, stdout) = classifier.report(case.report);

        assert_eq!(exit, case.expected_exit, "{}", case.name);
        for fragment in case.expected_fragments {
            assert!(stdout.contains(fragment), "{}: {stdout}", case.name);
        }
        if let Some(count) = case.expected_line_count {
            let non_empty_lines = stdout.lines().filter(|line| !line.is_empty()).count();
            assert_eq!(non_empty_lines, count, "{}: {stdout}", case.name);
        }
    }
}

#[test]
fn report_escapes_a_percent_in_a_not_reviewed_line() {
    let classifier = Classifier::new();
    classifier.run(Run {
        first: 1,
        result: Some(SESSION_RESULT),
        stderr_line: Some("stomme: 50%0A.rs: not reviewed (demo)"),
        ..Run::default()
    });

    let (exit, stdout) = classifier.report(Report {
        status: "1",
        ..Report::default()
    });

    assert_eq!(exit, 1);
    assert!(
        stdout.contains("::error::50%250A.rs: not reviewed (demo)"),
        "{stdout}"
    );
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
    assert_eq!(observation.coverage_verdict().as_deref(), Some("complete"));
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
    assert_eq!(observation.coverage_verdict().as_deref(), Some("failed"));
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
    assert_eq!(
        observation.coverage_verdict().as_deref(),
        Some("partial"),
        "the verdict follows the coverage file, not the job result"
    );
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
fn classifier_retries_a_provider_failure() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 2,
        result: Some(MIXED_RESULT),
        resume_result: Some(SESSION_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("0"));
    assert_eq!(observation.resume_calls(), 1);
    assert_eq!(observation.coverage_source().as_deref(), Some("resumed"));
    assert_eq!(observation.coverage_verdict().as_deref(), Some("complete"));
    assert!(observation.post_resumed());
    assert_eq!(observation.reason(), None);
    assert!(
        observation
            .coverage
            .expect("the resumed result writes coverage")
            .contains("ocr-resume.json")
    );
}

#[test]
fn classifier_fails_a_provider_failure_that_survives_the_retries() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 2,
        result: Some(MIXED_RESULT),
        resume: 1,
        resumed: 2,
        resume_result: Some(MIXED_RESULT),
        ..Run::default()
    });
    let reason = observation
        .reason()
        .expect("the failed run reports a reason");

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(observation.resume_calls(), 2, "the retries are bounded");
    assert_eq!(
        reason,
        "the review provider could not review 1 file after 3 attempts: src/b.rs"
    );
    assert_eq!(observation.coverage_source().as_deref(), Some("resumed"));
    assert_eq!(observation.coverage_verdict().as_deref(), Some("partial"));

    let (exit, stdout) = classifier.report(Report {
        status: "1",
        reason: &reason,
        ..Report::default()
    });

    assert_eq!(exit, 1);
    assert!(stdout.contains(&reason), "{stdout}");
    assert!(stdout.contains("not reviewed"), "{stdout}");
}

#[test]
fn classifier_does_not_retry_a_mixed_cause() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 2,
        result: Some(MIXED_CAUSE_RESULT),
        resume_result: Some(SESSION_RESULT),
        ..Run::default()
    });

    assert_eq!(
        observation.resume_calls(),
        0,
        "a file that failed for another reason is not retried"
    );
    assert_eq!(
        observation.reason().as_deref(),
        Some("provider, timeout"),
        "the reason names every failed classification"
    );
}

#[test]
fn classifier_does_not_retry_a_waived_file() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 2,
        result: Some(WAIVED_RESULT),
        resume_result: Some(SESSION_RESULT),
        ..Run::default()
    });

    assert_eq!(
        observation.resume_calls(),
        0,
        "a waived file needs no retry"
    );
}

#[test]
fn classifier_takes_the_reason_of_a_retry_that_wrote_no_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resume_silent: true,
        resume_unusable: true,
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(
        observation.reason().as_deref(),
        Some("the OCR run wrote no usable result: ocr: the provider request failed")
    );
    assert!(!observation.post_resumed());
}

#[test]
fn classifier_retries_a_failed_run_whose_files_all_failed_on_the_provider() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 1,
        result: Some(FAILED_PROVIDER_RESULT),
        resume_result: Some(SESSION_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("0"));
    assert_eq!(observation.resume_calls(), 1);
    assert_eq!(observation.coverage_source().as_deref(), Some("resumed"));
    assert!(observation.post_resumed());
}

#[test]
fn classifier_reports_the_coverage_error_of_a_rejected_first_result() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 1,
        result: Some("not json"),
        first_unusable: true,
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(observation.resume_calls(), 0);
    assert!(
        observation
            .reason()
            .as_deref()
            .expect("the rejected result reports a reason")
            .contains("is not a usable OCR result"),
        "{:?}",
        observation.reason()
    );
}

#[test]
fn classifier_does_not_retry_a_result_the_coverage_command_rejects() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        ocr_exit: Some(1),
        first: 1,
        result: Some(FAILED_PROVIDER_RESULT),
        first_unusable: true,
        resume_result: Some(SESSION_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(
        observation.resume_calls(),
        0,
        "an unclassified result is not retried"
    );
}

#[test]
fn classifier_resumes_a_budget_partial_once() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resumed: 2,
        resume_result: Some(BUDGET_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("2"));
    assert_eq!(
        observation.resume_calls(),
        1,
        "a budget partial is resumed once, because its budget is spent"
    );
    assert!(observation.post_resumed());
    assert_eq!(observation.coverage_verdict().as_deref(), Some("partial"));
}

#[test]
fn classifier_captures_the_retry_error() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resumed: 1,
        resume_stderr: Some("ocr: retrying a provider request"),
        ..Run::default()
    });

    assert_eq!(
        observation.resume_errors(),
        Some("ocr: retrying a provider request\n")
    );
}

#[test]
fn classifier_clears_the_retry_error_between_runs() {
    let classifier = Classifier::new();
    let first = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resume_stderr: Some("ocr: the first run failed"),
        ..Run::default()
    });
    let observation = classifier.run(Run {
        first: 1,
        result: Some(TIMEOUT_RESULT),
        ..Run::default()
    });

    assert_eq!(
        first.resume_errors(),
        Some("ocr: the first run failed\n"),
        "the first run wrote the error the second run must not read"
    );
    assert_eq!(
        observation.resume_errors(),
        None,
        "a run that does not retry reads no error from an earlier run"
    );
}

#[test]
fn classifier_keeps_a_parsed_result_over_the_cli_error() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resumed: 1,
        resume_stderr: Some("ocr: retrying a provider request"),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(
        observation.reason().as_deref(),
        Some("the OCR run recorded no failure reason"),
        "a result that parsed keeps its own reason"
    );
}

#[test]
fn classifier_takes_the_reason_from_an_unusable_retry() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        resume_result: Some(TIMEOUT_RESULT),
        resume_unusable: true,
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(
        observation.reason().as_deref(),
        Some("the global deadline elapsed"),
        "the failed retry names its own reason"
    );
    assert_eq!(observation.coverage_source().as_deref(), Some("first"));
    assert!(!observation.post_resumed());
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
        resume_result: Some(SESSION_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(
        observation.reason().as_deref(),
        Some("the global deadline elapsed")
    );
    assert_eq!(
        observation.resume_calls(),
        0,
        "a run failure needs a fix to the setup, not a retry"
    );
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
    assert_eq!(observation.coverage_verdict().as_deref(), Some("complete"));
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
fn classifier_resumes_from_the_scoped_base() {
    let classifier = Classifier::new();
    let observation = classifier.run(Run {
        first: 2,
        result: Some(SESSION_RESULT),
        range_from: SCOPED_BASE,
        ..Run::default()
    });
    let arguments = observation.ocr_arguments.expect("the resume ran");

    assert!(
        arguments.contains(&format!("--from {SCOPED_BASE}")),
        "the resume reviews the same synthetic base, because OCR seals the range: {arguments}"
    );
    assert!(arguments.contains("--to head-sha"), "{arguments}");
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
    assert_eq!(observation.coverage_verdict().as_deref(), Some("partial"));
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
        resume_result: Some(BUDGET_RESULT),
        ..Run::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.final_status().as_deref(), Some("1"));
    assert_eq!(observation.reason().as_deref(), Some("budget"));
    assert!(observation.resumed());
    assert_eq!(
        observation.resume_calls(),
        1,
        "a budget partial resumes once"
    );
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

    let (exit, stdout) = classifier.report(Report {
        status: "1",
        reason: "budget",
        ..Report::default()
    });

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
    assert_eq!(
        observation.coverage_verdict().as_deref(),
        Some("partial"),
        "the unusable resume keeps the first file's verdict"
    );
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
    assert!(
        observation
            .reason()
            .as_deref()
            .expect("the unusable resume reports a reason")
            .contains("is not a usable OCR result"),
        "the coverage command's own error explains the rejection"
    );
}

fn checkpoint_result(
    fingerprint: &str,
    head: &str,
    merge_base: &str,
    terminal_state: &str,
) -> String {
    serde_json::json!({
        "reason": "ok",
        "payload": {
            "v": 1,
            "pr": 7,
            "head": head,
            "base_ref": "main",
            "merge_base": merge_base,
            "terminal_state": terminal_state,
            "fingerprint": fingerprint,
            "run": "123",
        },
        "raw": "<!-- ocr-checkpoint:v1 marker -->",
    })
    .to_string()
}

struct Range {
    folder: TempDir,
    workspace: PathBuf,
}

struct RangeObservation {
    exit: i32,
    outputs: String,
    lookup: Option<serde_json::Value>,
    warnings: String,
}

impl RangeObservation {
    fn output(&self, key: &str) -> String {
        last_output(&self.outputs, key).unwrap_or_default()
    }
}

impl Range {
    fn new() -> Self {
        let folder = temporary_directory();
        let workspace = folder.path().join("workspace");
        write_posting_helper(&workspace, RANGE_HELPER);
        write_github_script(folder.path(), RANGE_STEP);

        Range { folder, workspace }
    }

    fn run(&self, checkpoint: Option<&str>, rule: Option<&str>) -> RangeObservation {
        let root = self.folder.path();
        let output_file = root.join("output");
        let lookup_file = root.join("lookup.json");
        let info_file = root.join("info.txt");
        let warning_file = root.join("warning.txt");
        let rule_file = self.workspace.join("docs/stomme/ocr-rules.json");
        for path in [&output_file, &lookup_file, &info_file, &warning_file] {
            remove_stale(path).expect("remove the stale range output");
        }
        match rule {
            Some(text) => {
                fs::create_dir_all(rule_file.parent().expect("the rule file has a folder"))
                    .expect("create the rule folder");
                fs::write(&rule_file, text).expect("write the rule file");
            }
            None => remove_stale(&rule_file).expect("remove the stale rule file"),
        }
        let checkpoint = checkpoint.map_or_else(
            || r#"{"reason":"no_summary_comment","payload":null,"raw":""}"#.to_string(),
            str::to_string,
        );
        let environment = [
            ("GITHUB_WORKSPACE", self.workspace.display().to_string()),
            ("PINNED_HELPER", posting_helper_path().display().to_string()),
            ("LOOKUP_LOG", lookup_file.display().to_string()),
            ("INFO_LOG", info_file.display().to_string()),
            ("WARNING_LOG", warning_file.display().to_string()),
            ("GITHUB_OUTPUT", output_file.display().to_string()),
            ("CHECKPOINT_RESULT", checkpoint),
            ("OCR_RULE", "docs/stomme/ocr-rules.json".to_string()),
            ("OCR_VERSION", "1.14.2".to_string()),
            ("OCR_LLM_URL", "https://api.morphllm.com/v1".to_string()),
            ("OCR_LLM_MODEL", "morph-dsv41flash-1017".to_string()),
            ("OCR_APP_SLUG", "github-actions".to_string()),
            ("PR_NUMBER", "7".to_string()),
            ("HEAD_SHA", HEAD_SHA.to_string()),
        ];
        let completed = run_github_script(root, &environment);

        assert!(
            completed.status.success(),
            "{}",
            String::from_utf8_lossy(&completed.stderr)
        );

        RangeObservation {
            exit: completed
                .status
                .code()
                .expect("the range script exits normally"),
            outputs: read(&output_file).unwrap_or_default(),
            lookup: read(&lookup_file)
                .map(|text| serde_json::from_str(&text).expect("the fake records JSON")),
            warnings: read(&warning_file).unwrap_or_default(),
        }
    }
}

#[test]
fn range_reviews_the_whole_pull_request_without_a_marker() {
    let range = Range::new();
    let observation = range.run(None, Some("rules"));

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.output("mode"), "full");
    assert_eq!(observation.output("reason"), "no_summary_comment");
    assert_eq!(observation.output("reviewed"), "");
    assert_eq!(
        observation.output("fingerprint").len(),
        16,
        "the run records the fingerprint it compared"
    );
}

#[test]
fn range_reviews_the_whole_pull_request_after_a_fingerprint_change() {
    let range = Range::new();
    let marker = checkpoint_result("0000000000000000", OTHER_HEAD, MERGE_BASE, "complete");
    let observation = range.run(Some(&marker), Some("rules"));

    assert_eq!(observation.output("mode"), "full");
    assert_eq!(
        observation.output("reason"),
        "config_changed",
        "a changed configuration reviews the whole pull request, moved base or not"
    );
    assert_eq!(observation.output("reviewed"), "");
}

#[test]
fn range_scopes_the_review_from_the_marker_head() {
    let range = Range::new();
    let fingerprint = range.run(None, Some("rules")).output("fingerprint");
    let marker = checkpoint_result(&fingerprint, OTHER_HEAD, MERGE_BASE, "complete");
    let observation = range.run(Some(&marker), Some("rules"));

    assert_eq!(observation.output("mode"), "scoped");
    assert_eq!(observation.output("reason"), "ok");
    assert_eq!(observation.output("reviewed"), OTHER_HEAD);
    assert_eq!(observation.output("reviewed_base"), MERGE_BASE);
    assert_eq!(
        observation.output("carry"),
        "<!-- ocr-checkpoint:v1 marker -->",
        "a run that does not advance re-emits the marker it read"
    );
    assert_eq!(
        observation.output("fingerprint"),
        fingerprint,
        "the run compares the same configuration twice"
    );
}

#[test]
fn range_scopes_the_review_after_a_moved_base() {
    let range = Range::new();
    let fingerprint = range.run(None, Some("rules")).output("fingerprint");
    let marker = checkpoint_result(&fingerprint, OTHER_HEAD, MERGE_BASE, "complete");
    let observation = range.run(Some(&marker), Some("rules"));

    assert_eq!(
        observation.output("reviewed_base"),
        MERGE_BASE,
        "the scope is measured from the base of the reviewed range, not the current one"
    );
    assert_eq!(observation.output("mode"), "scoped");
}

#[test]
fn range_skips_a_head_the_marker_already_records() {
    let range = Range::new();
    let fingerprint = range.run(None, Some("rules")).output("fingerprint");
    let marker = checkpoint_result(&fingerprint, HEAD_SHA, MERGE_BASE, "complete");
    let observation = range.run(Some(&marker), Some("rules"));

    assert_eq!(observation.output("mode"), "noop");
    assert_eq!(observation.output("reason"), "same_head_noop");
}

#[test]
fn range_reviews_the_whole_pull_request_for_an_incomplete_marker() {
    let range = Range::new();
    let fingerprint = range.run(None, Some("rules")).output("fingerprint");
    let marker = checkpoint_result(&fingerprint, OTHER_HEAD, MERGE_BASE, "partial");
    let observation = range.run(Some(&marker), Some("rules"));

    assert_eq!(observation.output("mode"), "full");
    assert_eq!(
        observation.output("reason"),
        "schema_invalid",
        "only a complete review narrows the next range"
    );
}

#[test]
fn range_reviews_the_whole_pull_request_without_the_rule_file() {
    let range = Range::new();
    let observation = range.run(None, None);

    assert_eq!(observation.output("mode"), "full");
    assert_eq!(observation.output("reason"), "rule_unreadable");
    assert_eq!(
        observation.output("fingerprint"),
        "",
        "a run that cannot hash the rules never advances the marker"
    );
    assert!(
        observation
            .warnings
            .contains("reviewing the whole pull request"),
        "{}",
        observation.warnings
    );
}

#[test]
fn range_passes_the_summary_lookup_its_options() {
    let range = Range::new();
    let observation = range.run(None, Some("rules"));

    assert_eq!(
        observation.lookup,
        Some(serde_json::json!({
            "owner": "owner",
            "repo": "repository",
            "prNumber": 7,
            "appSlug": "github-actions",
            "log": "function",
            "github": "object",
        }))
    );
}

struct Scope {
    folder: TempDir,
}

struct ScopeObservation {
    exit: i32,
    outputs: String,
    stdout: String,
    arguments: Option<String>,
    git_arguments: Option<String>,
    environment: Option<String>,
    result: Option<String>,
}

impl ScopeObservation {
    fn output(&self, key: &str) -> String {
        last_output(&self.outputs, key).unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
struct ScopeRun<'a> {
    base: &'a str,
    stderr: &'a str,
    fail_scope: bool,
    fail_fetch: bool,
}

impl Default for ScopeRun<'_> {
    fn default() -> Self {
        Self {
            base: "",
            stderr: "stomme: scope: 2 files, 3 added lines, 1 removed lines",
            fail_scope: false,
            fail_fetch: false,
        }
    }
}

impl Scope {
    fn new() -> Self {
        let folder = temporary_directory();
        let bin = folder.path().join("bin");
        fs::create_dir_all(&bin).expect("create the fake command folder");
        write_executable(&bin.join("git"), GIT_FAKE);
        write_executable(&bin.join("stomme"), SCOPE_FAKE);
        let script = step_block(OCR_REVIEW, SCOPE_STEP, "run")
            .replace("/tmp/", &format!("{}/", folder.path().display()));
        fs::write(folder.path().join("scope.sh"), script).expect("write the extracted script");

        Scope { folder }
    }

    fn run(&self, run: ScopeRun<'_>) -> ScopeObservation {
        let root = self.folder.path();
        let output_file = root.join("output");
        let environment_file = root.join("environment");
        let arguments = root.join("scope-arguments");
        let git_arguments = root.join("git-arguments");
        let result_file = root.join("ocr-result.json");
        for path in [
            &output_file,
            &environment_file,
            &arguments,
            &git_arguments,
            &result_file,
            &root.join("ocr-scope.err"),
        ] {
            remove_stale(path).expect("remove the stale scope output");
        }

        let completed = Command::new("/bin/bash")
            .arg("-e")
            .arg(root.join("scope.sh"))
            .current_dir(root)
            .env_clear()
            .env("PATH", search_path(&root.join("bin")))
            .env("GITHUB_OUTPUT", &output_file)
            .env("GITHUB_ENV", &environment_file)
            .env("REVIEWED", OTHER_HEAD)
            .env("REVIEWED_BASE", MERGE_BASE)
            .env("MERGE_BASE", "merge-base")
            .env("HEAD_SHA", HEAD_SHA)
            .env("SCOPE_ARGUMENTS", &arguments)
            .env("GIT_ARGUMENTS", &git_arguments)
            .env("SCOPE_BASE", run.base)
            .env("SCOPE_STDERR", run.stderr)
            .env("SCOPE_EXIT", if run.fail_scope { "1" } else { "0" })
            .env("GIT_FAIL", if run.fail_fetch { "1" } else { "0" })
            .output()
            .expect("run the extracted scope script");

        ScopeObservation {
            exit: completed
                .status
                .code()
                .expect("the scope script exits normally"),
            outputs: read(&output_file).unwrap_or_default(),
            stdout: String::from_utf8_lossy(&completed.stdout).into_owned(),
            arguments: read(&arguments),
            git_arguments: read(&git_arguments),
            environment: read(&environment_file),
            result: read(&result_file),
        }
    }
}

#[test]
fn scope_reviews_from_the_built_base() {
    let scope = Scope::new();
    let observation = scope.run(ScopeRun {
        base: SCOPED_BASE,
        ..ScopeRun::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.output("mode"), "scoped");
    assert_eq!(observation.output("base"), SCOPED_BASE);
    assert_eq!(
        observation.output("note"),
        "Scope since the last complete review: 2 files, 3 added lines, 1 removed lines"
    );
    assert_eq!(
        observation.git_arguments.as_deref(),
        Some(format!("fetch --filter=blob:none origin {OTHER_HEAD}\n").as_str()),
        "the marker head is fetched by SHA, because a force push removed its ref"
    );
    assert_eq!(
        observation.arguments.as_deref(),
        Some(
            format!(
                "ocr-scope --reviewed {OTHER_HEAD} --reviewed-base {MERGE_BASE} --base merge-base --head {HEAD_SHA}\n"
            )
            .as_str()
        )
    );
}

#[test]
fn scope_reviews_the_whole_pull_request_when_the_marker_head_is_unfetchable() {
    let scope = Scope::new();
    let observation = scope.run(ScopeRun {
        base: SCOPED_BASE,
        fail_fetch: true,
        ..ScopeRun::default()
    });

    assert_eq!(observation.exit, 0, "a failed fetch never fails the job");
    assert_eq!(observation.output("mode"), "full");
    assert_eq!(observation.output("base"), "");
    assert!(
        observation.stdout.contains("Cannot fetch the head"),
        "{}",
        observation.stdout
    );
    assert_eq!(
        observation.arguments, None,
        "an unfetchable head builds no scope"
    );
}

#[test]
fn scope_reviews_the_whole_pull_request_when_the_command_fails() {
    let scope = Scope::new();
    let observation = scope.run(ScopeRun {
        base: SCOPED_BASE,
        stderr: "stomme: the clone is shallow, so the review scope has no merge base",
        fail_scope: true,
        ..ScopeRun::default()
    });

    assert_eq!(
        observation.exit, 0,
        "a failed scope command never fails the job"
    );
    assert_eq!(observation.output("mode"), "full");
    assert!(
        observation
            .stdout
            .contains("the review scope has no merge base"),
        "{}",
        observation.stdout
    );
}

#[test]
fn scope_reviews_the_whole_pull_request_without_a_base() {
    let scope = Scope::new();
    let observation = scope.run(ScopeRun::default());

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.output("mode"), "full");
    assert!(
        observation.stdout.contains("printed no base commit"),
        "{}",
        observation.stdout
    );
}

#[test]
fn scope_records_no_new_lines_for_an_empty_scope() {
    let scope = Scope::new();
    let observation = scope.run(ScopeRun {
        base: "empty",
        stderr: "stomme: scope: 0 files, 0 added lines, 0 removed lines",
        ..ScopeRun::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.output("mode"), "empty");
    assert_eq!(
        observation.output("note"),
        "Scope since the last complete review: no new lines"
    );
    assert_eq!(
        observation.environment.as_deref(),
        Some("OCR_EXIT_CODE=0\n"),
        "an empty scope is a complete run of nothing"
    );

    let result: serde_json::Value = serde_json::from_str(
        observation
            .result
            .as_deref()
            .expect("an empty scope writes a result"),
    )
    .expect("the empty-scope result is JSON");

    assert_eq!(result["manifest"]["terminal_state"], "complete");
    assert_eq!(result["manifest"]["input"]["resolved_head"], HEAD_SHA);
    assert_eq!(
        result["manifest"]["coverage"]["selected"],
        serde_json::json!([])
    );
}

#[test]
fn empty_scope_result_advances_the_marker() {
    let scope = Scope::new();
    let observation = scope.run(ScopeRun {
        base: "empty",
        stderr: "stomme: scope: 0 files, 0 added lines, 0 removed lines",
        ..ScopeRun::default()
    });
    let result = observation.result.expect("an empty scope writes a result");
    let classifier = Classifier::new();
    let classified = classifier.run(Run {
        result: Some(&result),
        ..Run::default()
    });

    assert_eq!(
        classified.final_status().as_deref(),
        Some("0"),
        "a complete classification is what lets the post step advance the marker"
    );
    assert_eq!(classified.coverage_verdict().as_deref(), Some("complete"));
}

struct Review {
    folder: TempDir,
}

struct ReviewObservation {
    exit: i32,
    outputs: String,
    arguments: Option<String>,
    environment: Option<String>,
}

impl ReviewObservation {
    fn output(&self, key: &str) -> String {
        last_output(&self.outputs, key).unwrap_or_default()
    }
}

impl Review {
    fn new() -> Self {
        let folder = temporary_directory();
        let bin = folder.path().join("bin");
        fs::create_dir_all(&bin).expect("create the fake command folder");
        write_executable(&bin.join("ocr"), REVIEW_FAKE);
        let script = step_block(OCR_REVIEW, REVIEW_STEP, "run")
            .replace("/tmp/", &format!("{}/", folder.path().display()));
        fs::write(folder.path().join("review.sh"), script).expect("write the extracted script");

        Review { folder }
    }

    fn run(&self, from: &str, code: &str) -> ReviewObservation {
        let root = self.folder.path();
        let output_file = root.join("output");
        let environment_file = root.join("environment");
        let arguments = root.join("ocr-arguments");
        for path in [&output_file, &environment_file, &arguments] {
            remove_stale(path).expect("remove the stale review output");
        }

        let completed = Command::new("/bin/bash")
            .arg("-e")
            .arg(root.join("review.sh"))
            .current_dir(root)
            .env_clear()
            .env("PATH", search_path(&root.join("bin")))
            .env("GITHUB_OUTPUT", &output_file)
            .env("GITHUB_ENV", &environment_file)
            .env("MODEL_TOKEN", "model-token")
            .env("RANGE_FROM", from)
            .env("MERGE_BASE", MERGE_BASE)
            .env("HEAD_SHA", HEAD_SHA)
            .env("OCR_RULE", "docs/stomme/ocr-rules.json")
            .env("OCR_ARGUMENTS", &arguments)
            .env("OCR_REVIEW_EXIT", code)
            .output()
            .expect("run the extracted review script");

        ReviewObservation {
            exit: completed
                .status
                .code()
                .expect("the review script exits normally"),
            outputs: read(&output_file).unwrap_or_default(),
            arguments: read(&arguments),
            environment: read(&environment_file),
        }
    }
}

#[test]
fn review_runs_from_the_built_base() {
    let review = Review::new();
    let observation = review.run(SCOPED_BASE, "0");
    let arguments = observation.arguments.as_deref().expect("the review ran");

    assert_eq!(observation.exit, 0);
    assert!(
        arguments.contains(&format!("--from {SCOPED_BASE}")),
        "{arguments}"
    );
    assert!(
        arguments.contains(&format!("--to {HEAD_SHA}")),
        "{arguments}"
    );
    assert!(
        arguments.contains("--rule docs/stomme/ocr-rules.json"),
        "{arguments}"
    );
    assert_eq!(observation.output("exit_code"), "0");
    assert_eq!(
        observation.environment.as_deref(),
        Some("OCR_EXIT_CODE=0\n")
    );
}

#[test]
fn review_runs_from_the_merge_base_without_a_scope() {
    let review = Review::new();
    let observation = review.run("", "0");
    let arguments = observation.arguments.as_deref().expect("the review ran");

    assert!(
        arguments.contains(&format!("--from {MERGE_BASE}")),
        "{arguments}"
    );
    assert!(
        !arguments.contains("--from ") || arguments.contains(&format!("--from {MERGE_BASE}")),
        "a full review starts at the merge base"
    );
}

#[test]
fn review_records_a_provider_failure_without_failing_the_step() {
    let review = Review::new();
    let observation = review.run(SCOPED_BASE, "1");

    assert_eq!(
        observation.exit, 0,
        "the classification decides the job result, not the review step"
    );
    assert_eq!(observation.output("exit_code"), "1");
    assert_eq!(
        observation.environment.as_deref(),
        Some("OCR_EXIT_CODE=1\n")
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

#[derive(Clone, Copy)]
struct CoverageRun<'a> {
    coverage: Option<&'a str>,
    comment: Option<&'a str>,
    first_url: Option<&'a str>,
    resumed_url: Option<&'a str>,
    from_resume: bool,
    verdict: &'a str,
    head: &'a str,
    note: Option<&'a str>,
}

impl Default for CoverageRun<'_> {
    fn default() -> Self {
        Self {
            coverage: None,
            comment: None,
            first_url: None,
            resumed_url: None,
            from_resume: false,
            verdict: "complete",
            head: HEAD_SHA,
            note: None,
        }
    }
}

struct CoverageOutputs {
    coverage: PathBuf,
    comment: PathBuf,
    updated: PathBuf,
    get: PathBuf,
    update: PathBuf,
    info: PathBuf,
    warning: PathBuf,
    summary: PathBuf,
}

impl CoverageOutputs {
    fn new(root: &Path) -> Self {
        Self {
            coverage: root.join("ocr-coverage.md"),
            comment: root.join("comment.json"),
            updated: root.join("updated.txt"),
            get: root.join("get.txt"),
            update: root.join("update.txt"),
            info: root.join("info.txt"),
            warning: root.join("warning.txt"),
            summary: root.join("summary.txt"),
        }
    }

    fn all(&self) -> [&Path; 8] {
        [
            &self.coverage,
            &self.comment,
            &self.updated,
            &self.get,
            &self.update,
            &self.info,
            &self.warning,
            &self.summary,
        ]
    }

    fn observation(&self, completed: &Output) -> CoverageObservation {
        CoverageObservation {
            exit: completed
                .status
                .code()
                .expect("the coverage script exits normally"),
            updated: read(&self.updated),
            get: read(&self.get),
            update: read(&self.update),
            info: read(&self.info).unwrap_or_default(),
            warnings: read(&self.warning).unwrap_or_default(),
            summary: read(&self.summary),
        }
    }
}

impl Coverage {
    fn new() -> Self {
        let folder = temporary_directory();
        let workspace = folder.path().join("workspace");
        write_posting_helper(&workspace, POSTING_HELPER);

        let script = step_block(OCR_REVIEW, COVERAGE_STEP, "script")
            .replace("/tmp/", &format!("{}/", folder.path().display()));
        fs::write(
            folder.path().join("runner.js"),
            github_script_runner(&script),
        )
        .expect("write the runner");

        Coverage { folder, workspace }
    }

    fn run(&self, run: CoverageRun<'_>) -> CoverageObservation {
        let outputs = CoverageOutputs::new(self.folder.path());
        let completed = self.execute(run, &outputs);

        assert!(
            completed.status.success(),
            "{}",
            String::from_utf8_lossy(&completed.stderr)
        );

        outputs.observation(&completed)
    }

    fn run_refused(&self, run: CoverageRun<'_>) -> (String, CoverageObservation) {
        let outputs = CoverageOutputs::new(self.folder.path());
        let completed = self.execute(run, &outputs);

        assert!(
            !completed.status.success(),
            "the coverage script refuses the run"
        );

        let stderr = String::from_utf8_lossy(&completed.stderr).into_owned();

        (stderr, outputs.observation(&completed))
    }

    fn execute(&self, run: CoverageRun<'_>, outputs: &CoverageOutputs) -> Output {
        for path in outputs.all() {
            remove_stale(path).expect("remove the stale coverage output");
        }

        if let Some(coverage) = run.coverage {
            fs::write(&outputs.coverage, coverage).expect("write the coverage file");
        }

        if let Some(body) = run.comment {
            let comment = serde_json::json!({
                "id": 1,
                "html_url": "https://example.invalid/comment",
                "body": body,
            });
            fs::write(&outputs.comment, comment.to_string()).expect("write the comment file");
        }

        let updated = outputs.updated.display().to_string();
        let get = outputs.get.display().to_string();
        let update = outputs.update.display().to_string();
        let info = outputs.info.display().to_string();
        let warning = outputs.warning.display().to_string();
        let summary = outputs.summary.display().to_string();
        let environment = [
            ("GITHUB_WORKSPACE", self.workspace.display().to_string()),
            ("COMMENT_FILE", outputs.comment.display().to_string()),
            ("UPDATED_BODY", updated),
            ("GET_LOG", get),
            ("UPDATE_LOG", update),
            ("INFO_LOG", info),
            ("WARNING_LOG", warning),
            ("SUMMARY_LOG", summary),
            (
                "FIRST_COMMENT_URL",
                run.first_url.unwrap_or_default().to_string(),
            ),
            (
                "RESUMED_COMMENT_URL",
                run.resumed_url.unwrap_or_default().to_string(),
            ),
            (
                "COVERAGE_SOURCE",
                if run.from_resume { "resumed" } else { "first" }.to_string(),
            ),
            ("COVERAGE_VERDICT", run.verdict.to_string()),
            ("HEAD_SHA", run.head.to_string()),
            ("SCOPE_NOTE", run.note.unwrap_or_default().to_string()),
        ];

        run_github_script(self.folder.path(), &environment)
    }
}

#[test]
fn coverage_names_the_scope_above_the_coverage_section() {
    let coverage = Coverage::new();
    let note = "Scope since the last complete review: 2 files, 3 added lines, 1 removed lines";
    let observation = coverage.run(CoverageRun {
        coverage: Some("### Review coverage\n\nOCR reviewed 2 of 2 selected files."),
        comment: Some("<!-- ocr-summary -->\nSummary text"),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
        note: Some(note),
        ..CoverageRun::default()
    });

    let expected = format!(
        "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\n{}\n### Review scope\n\n{note}\n\n### Review coverage\n\nOCR reviewed 2 of 2 selected files.\n<!-- ocr-coverage:end -->\nSummary text",
        verdict_line(HEAD_SHA, "complete")
    );

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.updated.as_deref(), Some(expected.as_str()));
    assert!(
        observation
            .summary
            .expect("the section goes to the job summary")
            .contains(note)
    );
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

    let expected = format!(
        "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\n{}\nCovered: src/lib.rs\n<!-- ocr-coverage:end -->\nSummary text",
        verdict_line(HEAD_SHA, "complete")
    );

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.updated.as_deref(), Some(expected.as_str()));
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

    let expected = format!(
        "<!-- ocr-summary -->\n<!-- ocr-summary-run: 123 -->\n<!-- ocr-coverage:start -->\n{}\nCovered: src/lib.rs\n<!-- ocr-coverage:end -->\nSummary text",
        verdict_line(HEAD_SHA, "complete")
    );

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.updated.as_deref(), Some(expected.as_str()));
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

    let expected = format!(
        "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\n{}\nCovered: src/lib.rs\n<!-- ocr-coverage:end -->\nSummary",
        verdict_line(HEAD_SHA, "complete")
    );

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.updated.as_deref(), Some(expected.as_str()));
}

#[test]
fn coverage_replaces_the_whole_section_around_a_quoted_end_marker() {
    let coverage = Coverage::new();
    let fragment = "- `src/<!-- ocr-coverage:end -->.rs`: `unknown`, `no coverage record`";
    let section = format!(
        "<!-- ocr-coverage:start -->\n{}\n### Review coverage\n\n{fragment}\n<!-- ocr-coverage:end -->",
        verdict_line(OTHER_HEAD, "partial")
    );
    let findings = format!("Findings name {fragment}");
    let cases = [
        ("a marker in the section", "Summary text"),
        ("a marker below the section", findings.as_str()),
    ];

    for (name, after) in cases {
        let comment = format!("<!-- ocr-summary -->\n{section}\n{after}");
        let observation = coverage.run(CoverageRun {
            coverage: Some(&format!("### Review coverage\n\n{fragment}")),
            comment: Some(&comment),
            first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
            verdict: "partial",
            ..CoverageRun::default()
        });

        let expected = format!(
            "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\n{}\n### Review coverage\n\n{fragment}\n<!-- ocr-coverage:end -->\n{after}",
            verdict_line(HEAD_SHA, "partial")
        );

        assert_eq!(
            observation.updated.as_deref(),
            Some(expected.as_str()),
            "{name}"
        );
    }
}

#[test]
fn coverage_replaces_a_section_whose_marker_line_ends_with_whitespace() {
    let coverage = Coverage::new();
    let cases = [
        (
            "a CRLF body",
            "<!-- ocr-summary -->\r\n<!-- ocr-coverage:start -->\r\nold\r\n<!-- ocr-coverage:end -->\r\nSummary",
            "<!-- ocr-summary -->\r\n",
            "\r\nSummary",
        ),
        (
            "trailing whitespace after the end marker",
            "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\nold\n<!-- ocr-coverage:end -->  \nSummary",
            "<!-- ocr-summary -->\n",
            "  \nSummary",
        ),
        (
            "trailing whitespace before a CRLF ending",
            "<!-- ocr-summary -->\r\n<!-- ocr-coverage:start -->\r\nold\r\n<!-- ocr-coverage:end -->  \r\nSummary",
            "<!-- ocr-summary -->\r\n",
            "  \r\nSummary",
        ),
    ];

    for (name, comment, head, tail) in cases {
        let observation = coverage.run(CoverageRun {
            coverage: Some("Covered: src/lib.rs"),
            comment: Some(comment),
            first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
            ..CoverageRun::default()
        });

        let block = format!(
            "<!-- ocr-coverage:start -->\n{}\nCovered: src/lib.rs\n<!-- ocr-coverage:end -->",
            verdict_line(HEAD_SHA, "complete")
        );
        let expected = format!("{head}{block}{tail}");

        assert_eq!(observation.exit, 0, "{name}");
        assert_eq!(
            observation.updated.as_deref(),
            Some(expected.as_str()),
            "{name}"
        );
    }
}

#[test]
fn coverage_names_the_verdict_and_head_of_the_run() {
    let coverage = Coverage::new();
    let cases = [("partial", HEAD_SHA, "partial"), ("", OTHER_HEAD, "failed")];

    for (verdict, head, expected) in cases {
        let observation = coverage.run(CoverageRun {
            coverage: Some("Covered: src/lib.rs"),
            comment: Some("<!-- ocr-summary -->\nSummary text"),
            first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
            verdict,
            head,
            ..CoverageRun::default()
        });

        let updated = observation.updated.expect("the posted comment is updated");
        let line = verdict_line(head, expected);

        assert!(
            updated.contains(&format!("\n{line}\nCovered: src/lib.rs\n")),
            "{verdict:?}: {updated}"
        );
    }
}

#[test]
fn coverage_refuses_a_run_without_a_full_head_before_it_writes() {
    let coverage = Coverage::new();
    let (stderr, observation) = coverage.run_refused(CoverageRun {
        coverage: Some("Covered: src/lib.rs"),
        comment: Some("<!-- ocr-summary -->\nSummary text"),
        first_url: Some("https://github.com/owner/repository/pull/7#issuecomment-42"),
        head: "",
        ..CoverageRun::default()
    });

    assert!(
        stderr.contains("HEAD_SHA is not a full commit SHA"),
        "{stderr}"
    );
    assert_eq!(
        observation.summary, None,
        "no job summary for a refused run"
    );
    assert_eq!(
        observation.updated, None,
        "no comment update for a refused run"
    );
    assert_eq!(
        observation.update, None,
        "no comment update for a refused run"
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
        ..CoverageRun::default()
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

struct Post {
    folder: TempDir,
    workspace: PathBuf,
}

struct PostObservation {
    exit: i32,
    recorded: Option<serde_json::Value>,
}

#[derive(Clone, Copy)]
struct PostRun<'a> {
    result: &'a str,
    stderr: &'a str,
    fingerprint: &'a str,
    carry: &'a str,
}

impl Default for PostRun<'_> {
    fn default() -> Self {
        Self {
            result: "/tmp/ocr-result.json",
            stderr: "/tmp/ocr-stderr.log",
            fingerprint: "",
            carry: "",
        }
    }
}

impl Post {
    fn new(step: &str) -> Self {
        let folder = temporary_directory();
        let workspace = folder.path().join("workspace");
        write_posting_helper(&workspace, POSTING_HELPER_RECORD);
        write_github_script(folder.path(), step);

        Post { folder, workspace }
    }

    fn run(&self, run: PostRun<'_>) -> PostObservation {
        let root = self.folder.path();
        let record = root.join("record.json");
        remove_stale(&record).expect("remove the stale record");

        let environment = [
            ("GITHUB_WORKSPACE", self.workspace.display().to_string()),
            ("PINNED_HELPER", posting_helper_path().display().to_string()),
            ("RECORD_LOG", record.display().to_string()),
            ("PR_NUMBER", "7".to_string()),
            ("OCR_STICKY_SUMMARY", "true".to_string()),
            ("OCR_INCREMENTAL", "true".to_string()),
            ("OCR_CHECKPOINT_RANGE", "true".to_string()),
            ("OCR_APP_SLUG", "github-actions".to_string()),
            ("RESOLVE_OUTDATED", "true".to_string()),
            ("BASE_REF", "main".to_string()),
            ("MERGE_BASE", MERGE_BASE.to_string()),
            ("RESULT_PATH", run.result.to_string()),
            ("STDERR_PATH", run.stderr.to_string()),
            ("CHECKPOINT_FINGERPRINT", run.fingerprint.to_string()),
            ("CHECKPOINT_CARRY", run.carry.to_string()),
        ];
        let completed = run_github_script(root, &environment);

        assert!(
            completed.status.success(),
            "{}",
            String::from_utf8_lossy(&completed.stderr)
        );

        PostObservation {
            exit: completed.status.code().expect("the script exits normally"),
            recorded: read(&record)
                .map(|text| serde_json::from_str(&text).expect("the helper records JSON")),
        }
    }
}

#[test]
fn the_two_posts_share_one_script() {
    assert_eq!(
        step_block(OCR_REVIEW, POST_STEP, "script"),
        step_block(OCR_REVIEW, RESUME_POST_STEP, "script"),
        "the first post and the resumed post differ only in their inputs"
    );
}

#[test]
fn resumed_post_passes_the_posting_helper_its_options() {
    let post = Post::new(RESUME_POST_STEP);
    let observation = post.run(PostRun {
        result: "/tmp/ocr-resume.json",
        stderr: "/tmp/ocr-resume.err",
        ..PostRun::default()
    });

    assert_eq!(observation.exit, 0);
    assert_eq!(
        observation.recorded,
        Some(serde_json::json!({
            "github": "object",
            "context": "object",
            "core": "object",
            "fs": "object",
            "prNumber": 7,
            "resultPath": "/tmp/ocr-resume.json",
            "stderrPath": "/tmp/ocr-resume.err",
            "stickySummary": true,
            "incremental": true,
            "checkpointEnabled": true,
            "checkpointCarry": "",
            "checkpointBaseRef": "main",
            "checkpointMergeBase": "aabbccddeeff00112233445566778899aabbccdd",
            "checkpointFingerprint": "",
            "checkpointNoop": false,
            "rangeMode": "",
            "rangeFrom": "",
            "rangeTo": "",
            "resolveOutdated": "true",
            "appSlug": "github-actions",
        }))
    );
}

#[test]
fn post_passes_the_posting_helper_its_options() {
    let post = Post::new(POST_STEP);
    let observation = post.run(PostRun::default());
    let recorded = observation
        .recorded
        .expect("the helper records its options");

    assert_eq!(observation.exit, 0);
    assert_eq!(recorded["resultPath"], "/tmp/ocr-result.json");
    assert_eq!(recorded["stderrPath"], "/tmp/ocr-stderr.log");
    assert_eq!(recorded["prNumber"], 7);
    assert_eq!(recorded["stickySummary"], true);
    assert_eq!(recorded["checkpointEnabled"], true);
    assert_eq!(
        recorded["checkpointMergeBase"],
        "aabbccddeeff00112233445566778899aabbccdd"
    );
}

#[test]
fn post_advances_the_marker_of_a_complete_review() {
    let post = Post::new(POST_STEP);
    let observation = post.run(PostRun {
        fingerprint: "0123456789abcdef",
        carry: "<!-- ocr-checkpoint:v1 marker -->",
        ..PostRun::default()
    });
    let recorded = observation
        .recorded
        .expect("the helper records its options");

    assert_eq!(recorded["checkpointFingerprint"], "0123456789abcdef");
    assert_eq!(
        recorded["checkpointCarry"], "<!-- ocr-checkpoint:v1 marker -->",
        "the marker read this run is re-emitted when the review does not advance"
    );
}

#[test]
fn post_keeps_the_old_marker_of_a_partial_review() {
    let post = Post::new(RESUME_POST_STEP);
    let observation = post.run(PostRun {
        result: "/tmp/ocr-resume.json",
        stderr: "/tmp/ocr-resume.err",
        fingerprint: "",
        carry: "<!-- ocr-checkpoint:v1 marker -->",
    });
    let recorded = observation
        .recorded
        .expect("the helper records its options");

    assert_eq!(
        recorded["checkpointFingerprint"], "",
        "an empty fingerprint keeps the previous marker, so the next range only widens"
    );
    assert_eq!(
        recorded["checkpointCarry"],
        "<!-- ocr-checkpoint:v1 marker -->"
    );
    assert_eq!(recorded["resultPath"], "/tmp/ocr-resume.json");
}

struct CheckpointedCoverage {
    folder: TempDir,
    workspace: PathBuf,
}

struct CheckpointedCoverageObservation {
    exit: i32,
    verdict: Option<String>,
    lookup: Option<serde_json::Value>,
    warnings: String,
    summary: Option<String>,
}

impl CheckpointedCoverage {
    fn new() -> Self {
        let folder = temporary_directory();
        let workspace = folder.path().join("workspace");
        write_posting_helper(&workspace, CHECKPOINT_HELPER);
        write_github_script(folder.path(), CHECKPOINT_STEP);

        CheckpointedCoverage { folder, workspace }
    }

    fn run(&self, body: Option<&str>) -> CheckpointedCoverageObservation {
        self.run_with(body, "true")
    }

    fn run_with(&self, body: Option<&str>, sticky: &str) -> CheckpointedCoverageObservation {
        let root = self.folder.path();
        let comment_file = root.join("comment.json");
        let output_file = root.join("output");
        let info_file = root.join("info.txt");
        let warning_file = root.join("warning.txt");
        let summary_file = root.join("summary.txt");
        let lookup_file = root.join("lookup.json");
        for path in [
            &comment_file,
            &output_file,
            &info_file,
            &warning_file,
            &summary_file,
            &lookup_file,
        ] {
            remove_stale(path).expect("remove the stale checkpointed output");
        }

        if let Some(body) = body {
            let comment = serde_json::json!({ "id": 1, "body": body });
            fs::write(&comment_file, comment.to_string()).expect("write the comment file");
        }

        let environment = [
            ("GITHUB_WORKSPACE", self.workspace.display().to_string()),
            ("PINNED_HELPER", posting_helper_path().display().to_string()),
            ("COMMENT_FILE", comment_file.display().to_string()),
            ("GITHUB_OUTPUT", output_file.display().to_string()),
            ("INFO_LOG", info_file.display().to_string()),
            ("WARNING_LOG", warning_file.display().to_string()),
            ("SUMMARY_LOG", summary_file.display().to_string()),
            ("LOOKUP_LOG", lookup_file.display().to_string()),
            ("PR_NUMBER", "7".to_string()),
            ("OCR_APP_SLUG", "github-actions".to_string()),
            ("OCR_STICKY_SUMMARY", sticky.to_string()),
            ("HEAD_SHA", HEAD_SHA.to_string()),
        ];
        let completed = run_github_script(root, &environment);

        assert!(
            completed.status.success(),
            "{}",
            String::from_utf8_lossy(&completed.stderr)
        );

        CheckpointedCoverageObservation {
            exit: completed.status.code().expect("the script exits normally"),
            verdict: read(&output_file).and_then(|text| last_output(&text, "verdict")),
            lookup: read(&lookup_file)
                .map(|text| serde_json::from_str(&text).expect("the fake records JSON")),
            warnings: read(&warning_file).unwrap_or_default(),
            summary: read(&summary_file),
        }
    }
}

fn coverage_section(verdict: &str) -> String {
    format!(
        "<!-- ocr-summary -->\n\
         <!-- ocr-summary-run: 123 -->\n\
         <!-- ocr-coverage:start -->\n\
         {}\n\
         ### Review coverage\n\
         \n\
         OCR reviewed 1 of 1 selected file.\n\
         <!-- ocr-coverage:end -->\n\
         Summary text",
        verdict_line(HEAD_SHA, verdict)
    )
}

#[test]
fn checkpointed_coverage_passes_the_summary_lookup_its_options() {
    let checkpoint = CheckpointedCoverage::new();
    let observation = checkpoint.run(Some(&coverage_section("complete")));

    assert_eq!(
        observation.lookup,
        Some(serde_json::json!({
            "owner": "owner",
            "repo": "repository",
            "prNumber": 7,
            "sticky": true,
            "appSlug": "github-actions",
            "log": "function",
            "github": "object",
        }))
    );
}

#[test]
fn checkpointed_coverage_follows_the_sticky_summary_setting() {
    let checkpoint = CheckpointedCoverage::new();
    let observation = checkpoint.run_with(Some(&coverage_section("complete")), "false");

    assert_eq!(
        observation.lookup.expect("the fake records the lookup")["sticky"],
        false
    );
}

#[test]
fn a_no_op_run_passes_with_a_complete_coverage_section() {
    let checkpoint = CheckpointedCoverage::new();
    let classifier = Classifier::new();
    let observation = checkpoint.run(Some(&coverage_section("complete")));

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.verdict.as_deref(), Some("complete"));
    assert!(
        observation
            .summary
            .expect("the section goes to the job summary")
            .contains("### Review coverage")
    );

    let (exit, stdout) = classifier.report(Report {
        range_mode: "noop",
        status: "",
        verdict: "complete",
        ..Report::default()
    });

    assert_eq!(exit, 0);
    assert!(
        stdout.contains("every selected file as reviewed"),
        "{stdout}"
    );
}

#[test]
fn a_no_op_run_fails_with_a_partial_coverage_section() {
    let checkpoint = CheckpointedCoverage::new();
    let classifier = Classifier::new();
    let observation = checkpoint.run(Some(&coverage_section("partial")));

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.verdict.as_deref(), Some("partial"));

    let (exit, stdout) = classifier.report(Report {
        range_mode: "noop",
        status: "",
        verdict: "partial",
        ..Report::default()
    });

    assert_eq!(exit, 1);
    assert!(stdout.contains("unreviewed files"), "{stdout}");
    assert!(stdout.contains("Push a new commit"), "{stdout}");
}

#[test]
fn a_no_op_run_fails_with_a_failed_coverage_section() {
    let checkpoint = CheckpointedCoverage::new();
    let classifier = Classifier::new();
    let observation = checkpoint.run(Some(&coverage_section("failed")));

    assert_eq!(observation.exit, 0);
    assert_eq!(observation.verdict.as_deref(), Some("failed"));

    let (exit, stdout) = classifier.report(Report {
        range_mode: "noop",
        status: "",
        verdict: "failed",
        ..Report::default()
    });

    assert_eq!(exit, 1);
    assert!(stdout.contains("failed run"), "{stdout}");
    assert!(stdout.contains("Push a new commit"), "{stdout}");
}

#[test]
fn checkpointed_coverage_reads_the_section_up_to_its_own_end_marker() {
    let checkpoint = CheckpointedCoverage::new();
    let quoted = "- `src/<!-- ocr-coverage:end -->.rs`: `unknown`, `no coverage record`";
    let section = format!(
        "<!-- ocr-summary -->\n<!-- ocr-summary-run: 123 -->\n<!-- ocr-coverage:start -->\n{}\n### Review coverage\n\n{quoted}\n<!-- ocr-coverage:end -->",
        verdict_line(HEAD_SHA, "complete")
    );
    let findings = format!("Findings name {quoted}");
    let cases = [
        (
            "a marker in the section",
            format!("{section}\nSummary text"),
        ),
        (
            "a marker below the section",
            format!("{section}\n{findings}"),
        ),
        (
            "a section that ends with trailing whitespace",
            format!("{section}  \nSummary text"),
        ),
    ];
    let expected = format!(
        "{}\n### Review coverage\n\n{quoted}",
        verdict_line(HEAD_SHA, "complete")
    );

    for (name, body) in cases {
        let observation = checkpoint.run(Some(&body));

        assert_eq!(observation.exit, 0, "{name}");
        assert_eq!(
            observation.verdict.as_deref(),
            Some("complete"),
            "{name}: {}",
            observation.warnings
        );
        assert_eq!(
            observation.summary.as_deref(),
            Some(expected.as_str()),
            "{name}: the section stops at its own end marker"
        );
    }
}

#[test]
fn a_no_op_run_fails_without_a_readable_coverage_section() {
    let checkpoint = CheckpointedCoverage::new();
    let classifier = Classifier::new();
    let cases = [
        ("no summary comment", None, "missing", "no coverage section"),
        (
            "a section without a verdict",
            Some(
                "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\n### Review coverage\n<!-- ocr-coverage:end -->"
                    .to_string(),
            ),
            "no-verdict",
            "names no verdict",
        ),
        (
            "a start marker without an end marker",
            Some("<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\n### Review coverage\n".to_string()),
            "missing",
            "no coverage section",
        ),
        (
            "a section that covers another head",
            Some(format!(
                "<!-- ocr-summary -->\n<!-- ocr-coverage:start -->\n{}\n### Review coverage\n<!-- ocr-coverage:end -->",
                verdict_line(OTHER_HEAD, "complete")
            )),
            "other-head",
            "covers another head",
        ),
    ];

    for (name, body, expected_verdict, expected_detail) in cases {
        let observation = checkpoint.run(body.as_deref());

        assert_eq!(observation.exit, 0, "{name}");
        assert_eq!(
            observation.verdict.as_deref(),
            Some(expected_verdict),
            "{name}"
        );
        assert_eq!(observation.summary, None, "{name}");
        assert!(!observation.warnings.is_empty(), "{name}");

        let (exit, stdout) = classifier.report(Report {
            range_mode: "noop",
            status: "",
            verdict: expected_verdict,
            ..Report::default()
        });

        assert_eq!(exit, 1, "{name}");
        assert!(stdout.contains(expected_detail), "{name}: {stdout}");
        assert!(stdout.contains("Push a new commit"), "{name}: {stdout}");
    }
}
