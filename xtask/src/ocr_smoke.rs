use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::Context as _;
use serde::Deserialize;
use tempfile::TempDir;

use crate::git_isolation::isolate_git;

mod scope;

const USAGE: &str = "Usage:\n  cargo xtask ocr-smoke";

const OCR_BINARY: &str = "node_modules/.bin/ocr";
const RULES: &str = "docs/stomme/ocr-rules.json";
const CUSTOM_RULE_SOURCE: &str = "Custom (--rule)";
const COMMIT_DATE: &str = "2020-01-01T00:00:00Z";

const BASE_LIB: &str = "\
pub fn add(left: i32, right: i32) -> i32 {
    left + right
}
";

const HEAD_LIB: &str = "\
pub fn add(left: i32, right: i32) -> i32 {
    left.wrapping_add(right)
}
";

const RUST_TEST: &str = "\
#[test]
fn adds_two_numbers() {
    assert_eq!(2, demo::add(1, 1));
}
";

const TYPESCRIPT_TEST: &str = "\
import { add } from '../src/add.js';

test('adds two numbers', () => {
  expect(add(1, 1)).toBe(2);
});
";

const TYPESCRIPT_SOURCE_TEST: &str = "\
test('adds two numbers', () => {
  expect(1 + 1).toBe(2);
});
";

const FIXTURE_LIB: &str = "\
pub fn sample() -> bool {
    true
}
";

const BASE_FILES: &[(&str, &str)] = &[("crates/demo/src/lib.rs", BASE_LIB)];

const HEAD_FILES: &[(&str, &str)] = &[
    ("crates/demo/src/lib.rs", HEAD_LIB),
    ("crates/demo/tests/add.rs", RUST_TEST),
    ("packages/demo/tests/add.test.ts", TYPESCRIPT_TEST),
    ("packages/demo/src/add.test.ts", TYPESCRIPT_SOURCE_TEST),
    (
        "crates/stomme-cli/tests/fixtures/sample/src/lib.rs",
        FIXTURE_LIB,
    ),
];

struct Case {
    name: &'static str,
    reviewed: &'static [&'static str],
    excluded: &'static [(&'static str, &'static str)],
    precedence: &'static [(&'static str, &'static str)],
}

const REPOSITORY_CASE: Case = Case {
    name: "repository",
    reviewed: &[
        "crates/demo/src/lib.rs",
        "crates/demo/tests/add.rs",
        "packages/demo/tests/add.test.ts",
        "packages/demo/src/add.test.ts",
    ],
    excluded: &[(
        "crates/stomme-cli/tests/fixtures/sample/src/lib.rs",
        "user_exclude",
    )],
    precedence: &[
        ("crates/demo/tests/add.rs", "**/*.rs"),
        (
            "packages/demo/tests/add.test.ts",
            "packages/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
        ),
        (
            "packages/demo/src/add.test.ts",
            "packages/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
        ),
    ],
};

const TYPESCRIPT_CASE: Case = Case {
    name: "typescript consumer",
    reviewed: &["packages/demo/tests/add.test.ts"],
    excluded: &[],
    precedence: &[(
        "packages/demo/tests/add.test.ts",
        "**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
    )],
};

pub(super) struct CaseOutcome {
    name: &'static str,
    admission: Vec<String>,
    precedence: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Preview {
    files: Vec<PreviewFile>,
}

impl Preview {
    fn file(&self, path: &str) -> Option<&PreviewFile> {
        self.files.iter().find(|file| file.path == path)
    }
}

#[derive(Debug, Deserialize)]
struct PreviewFile {
    path: String,
    will_review: bool,
    #[serde(default)]
    exclude_reason: Option<String>,
}

#[derive(Debug)]
struct RuleCheck {
    source: String,
    pattern: String,
}

pub(super) struct CommandOutput {
    stdout: String,
    stderr: String,
}

pub(super) struct Ocr {
    binary: PathBuf,
    root: PathBuf,
    home: PathBuf,
}

struct Repository {
    folder: TempDir,
    base: String,
    head: String,
}

#[expect(
    clippy::print_stderr,
    reason = "the smoke results are the command's output"
)]
pub(crate) fn run(root: &Path, arguments: &[String]) -> ExitCode {
    if !arguments.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    }

    let outcomes = match check(root) {
        Ok(outcomes) => outcomes,
        Err(error) => {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
    };

    exit_code(&outcomes)
}

fn exit_code(outcomes: &[CaseOutcome]) -> ExitCode {
    if report(outcomes) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn check(root: &Path) -> anyhow::Result<Vec<CaseOutcome>> {
    let binary = root.join(OCR_BINARY);
    if !binary.is_file() {
        anyhow::bail!("{OCR_BINARY} is missing; run pnpm install --frozen-lockfile");
    }

    let home = temporary_folder("stomme-ocr-smoke-home-")?;
    let repository = build_repository()?;
    let typescript = build_typescript_consumer(root)?;
    let ocr = Ocr {
        binary,
        root: root.to_path_buf(),
        home: home.path().to_path_buf(),
    };

    let cases = [
        (&REPOSITORY_CASE, root.join(RULES)),
        (&TYPESCRIPT_CASE, typescript.path().join(RULES)),
    ];
    let mut outcomes = Vec::new();

    for (case, rules) in cases {
        outcomes.push(run_case(
            &ocr,
            repository.folder.path(),
            &rules,
            &repository.base,
            &repository.head,
            case,
        )?);
    }

    outcomes.push(scope::run_scope_case(&ocr)?);

    Ok(outcomes)
}

fn run_case(
    ocr: &Ocr,
    scratch: &Path,
    rules: &Path,
    base: &str,
    head: &str,
    case: &Case,
) -> anyhow::Result<CaseOutcome> {
    let preview_output = run_ocr(ocr, &preview_arguments(scratch, rules, base, head))?;
    let preview = parse_preview(&preview_output.stdout, &preview_output.stderr)?;
    let admission = admission_failures(case.name, &preview, case.reviewed, case.excluded);
    let mut precedence = Vec::new();

    for (path, expected_pattern) in case.precedence {
        let check_output = run_ocr(ocr, &rules_check_arguments(scratch, rules, path))?;
        let checked = parse_rule_check(&check_output.stdout, &check_output.stderr)?;
        precedence.extend(precedence_failure(
            case.name,
            path,
            &checked,
            expected_pattern,
        ));
    }

    Ok(CaseOutcome {
        name: case.name,
        admission,
        precedence,
    })
}

fn admission_failures(
    case: &str,
    preview: &Preview,
    reviewed: &[&str],
    excluded: &[(&str, &str)],
) -> Vec<String> {
    let mut failures = Vec::new();

    for path in reviewed {
        match preview.file(path) {
            None => failures.push(not_in_preview(case, path)),
            Some(file) if !file.will_review => failures.push(format!(
                "ocr-smoke: {case}: admission failed: {path} expected reviewed, got excluded ({})",
                exclude_reason(file)
            )),
            Some(_) => {}
        }
    }

    for (path, expected) in excluded {
        match preview.file(path) {
            None => failures.push(not_in_preview(case, path)),
            Some(file) if file.will_review => failures.push(format!(
                "ocr-smoke: {case}: admission failed: {path} expected excluded ({expected}), got reviewed"
            )),
            Some(file) if exclude_reason(file) != *expected => failures.push(format!(
                "ocr-smoke: {case}: admission failed: {path} expected excluded ({expected}), got excluded ({})",
                exclude_reason(file)
            )),
            Some(_) => {}
        }
    }

    failures
}

fn precedence_failure(
    case: &str,
    path: &str,
    checked: &RuleCheck,
    expected_pattern: &str,
) -> Option<String> {
    let matches = checked.source == CUSTOM_RULE_SOURCE && checked.pattern == expected_pattern;
    if matches {
        return None;
    }

    Some(format!(
        "ocr-smoke: {case}: precedence failed: {path} expected {CUSTOM_RULE_SOURCE} pattern {expected_pattern}, got {} pattern {}",
        checked.source, checked.pattern
    ))
}

fn not_in_preview(case: &str, path: &str) -> String {
    format!("ocr-smoke: {case}: admission failed: {path} not in the preview")
}

fn exclude_reason(file: &PreviewFile) -> &str {
    file.exclude_reason.as_deref().unwrap_or("no reason given")
}

#[expect(
    clippy::print_stderr,
    reason = "the smoke results are the command's output"
)]
fn report(outcomes: &[CaseOutcome]) -> bool {
    let mut passed = true;

    for outcome in outcomes {
        if outcome.admission.is_empty() {
            eprintln!("ocr-smoke: {}: admission passed", outcome.name);
        } else {
            passed = false;
            for failure in &outcome.admission {
                eprintln!("{failure}");
            }
        }

        if outcome.precedence.is_empty() {
            eprintln!("ocr-smoke: {}: precedence passed", outcome.name);
        } else {
            passed = false;

            for failure in &outcome.precedence {
                eprintln!("{failure}");
            }
        }
    }

    passed
}

pub(super) fn preview_arguments(
    scratch: &Path,
    rules: &Path,
    base: &str,
    head: &str,
) -> Vec<String> {
    vec![
        "review".to_string(),
        "--repo".to_string(),
        display_path(scratch),
        "--rule".to_string(),
        display_path(rules),
        "--from".to_string(),
        base.to_string(),
        "--to".to_string(),
        head.to_string(),
        "--preview".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ]
}

fn rules_check_arguments(scratch: &Path, rules: &Path, path: &str) -> Vec<String> {
    vec![
        "rules".to_string(),
        "check".to_string(),
        "--repo".to_string(),
        display_path(scratch),
        "--rule".to_string(),
        display_path(rules),
        path.to_string(),
    ]
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

pub(super) fn run_ocr(ocr: &Ocr, arguments: &[String]) -> anyhow::Result<CommandOutput> {
    let description = format!("ocr {}", arguments.join(" "));
    let output = ocr_command(ocr, arguments)
        .output()
        .with_context(|| format!("Could not run {description}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        anyhow::bail!("{description} failed with {}:\n{stderr}", output.status);
    }

    let stdout = String::from_utf8(output.stdout)
        .with_context(|| format!("{description} printed output that is not UTF-8:\n{stderr}"))?;

    Ok(CommandOutput { stdout, stderr })
}

fn ocr_command(ocr: &Ocr, arguments: &[String]) -> Command {
    let mut command = Command::new(&ocr.binary);

    command
        .current_dir(&ocr.root)
        .env("OCR_NO_UPDATE", "1")
        .env("HOME", &ocr.home)
        .env("XDG_CONFIG_HOME", &ocr.home)
        .args(["--color", "never"])
        .args(arguments);
    isolate_git(&mut command);

    command
}

pub(super) fn parse_preview(text: &str, stderr: &str) -> anyhow::Result<Preview> {
    serde_json::from_str(text).with_context(|| {
        format!("ocr review --preview printed JSON that is not a preview; stderr:\n{stderr}")
    })
}

fn parse_rule_check(text: &str, stderr: &str) -> anyhow::Result<RuleCheck> {
    Ok(RuleCheck {
        source: field(text, "Source: ", stderr)?,
        pattern: field(text, "Pattern: ", stderr)?,
    })
}

fn field(text: &str, prefix: &str, stderr: &str) -> anyhow::Result<String> {
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .with_context(|| format!("ocr rules check printed no {prefix}line; stderr:\n{stderr}"))?;

    Ok(value.trim().to_string())
}

fn build_repository() -> anyhow::Result<Repository> {
    let folder = temporary_folder("stomme-ocr-smoke-repository-")?;
    let path = folder.path();

    init_repository(path)?;

    write_files(path, BASE_FILES)?;
    git_text(path, &["add", "-A"])?;
    let base_tree = git_text(path, &["write-tree"])?;
    let base = git_text(path, &["commit-tree", &base_tree, "-m", "base"])?;

    write_files(path, HEAD_FILES)?;
    git_text(path, &["add", "-A"])?;
    let head_tree = git_text(path, &["write-tree"])?;
    let head = git_text(
        path,
        &["commit-tree", &head_tree, "-p", &base, "-m", "head"],
    )?;

    Ok(Repository { folder, base, head })
}

fn build_typescript_consumer(root: &Path) -> anyhow::Result<TempDir> {
    let folder = temporary_folder("stomme-ocr-smoke-typescript-")?;
    let path = folder.path();

    init_repository(path)?;
    let version = stomme_version(root)?;
    let configuration = format!("schema = 1\nversion = \"{version}\"\nlanguage = \"typescript\"\n");
    fs::write(path.join("stomme.toml"), configuration)
        .context("Could not write the TypeScript consumer's stomme.toml")?;
    sync_typescript_consumer(root, path)?;

    Ok(folder)
}

fn stomme_version(root: &Path) -> anyhow::Result<String> {
    let path = root.join("stomme.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("Could not read {}", path.display()))?;
    let document: toml_edit::DocumentMut = text
        .parse()
        .with_context(|| format!("{} is not valid TOML", path.display()))?;
    let version = document
        .get("version")
        .and_then(|item| item.as_str())
        .with_context(|| format!("{} has no version", path.display()))?;

    Ok(version.to_string())
}

fn sync_typescript_consumer(root: &Path, folder: &Path) -> anyhow::Result<()> {
    let output = sync_command(root, folder)
        .output()
        .context("Could not run cargo run -p stomme -- sync")?;

    if !output.status.success() {
        anyhow::bail!(
            "cargo run -p stomme -- sync failed with {}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn sync_command(root: &Path, folder: &Path) -> Command {
    let mut command = Command::new("cargo");

    command
        .args(["run", "--locked", "--quiet", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .args(["-p", "stomme", "--", "sync"])
        .current_dir(folder);
    isolate_git(&mut command);

    command
}

pub(super) fn init_repository(folder: &Path) -> anyhow::Result<()> {
    git_text(folder, &["init", "-q"])?;

    Ok(())
}

pub(super) fn write_files(folder: &Path, files: &[(&str, &str)]) -> anyhow::Result<()> {
    for (relative, contents) in files {
        let path = folder.join(relative);
        let parent = path.parent().expect("a file path has a parent");
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create {}", parent.display()))?;
        fs::write(&path, contents)
            .with_context(|| format!("Could not write {}", path.display()))?;
    }

    Ok(())
}

fn git_command(folder: &Path) -> Command {
    let mut command = Command::new("git");

    command
        .current_dir(folder)
        .env("GIT_AUTHOR_NAME", "stomme")
        .env("GIT_AUTHOR_EMAIL", "stomme@example.com")
        .env("GIT_COMMITTER_NAME", "stomme")
        .env("GIT_COMMITTER_EMAIL", "stomme@example.com")
        .env("GIT_AUTHOR_DATE", COMMIT_DATE)
        .env("GIT_COMMITTER_DATE", COMMIT_DATE);
    isolate_git(&mut command);

    command
}

pub(super) fn git_text(folder: &Path, arguments: &[&str]) -> anyhow::Result<String> {
    let mut command = git_command(folder);

    git_text_from(command.args(arguments), arguments)
}

fn git_text_from(command: &mut Command, arguments: &[&str]) -> anyhow::Result<String> {
    let description = format!("git {}", arguments.join(" "));
    let output = command
        .output()
        .with_context(|| format!("Could not run {description}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{description} failed with {}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let text = String::from_utf8(output.stdout).context("git printed output that is not UTF-8")?;

    Ok(text.trim().to_string())
}

pub(super) fn temporary_folder(prefix: &str) -> anyhow::Result<TempDir> {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .context("Could not create a temporary folder")
}

#[cfg(test)]
mod tests;
