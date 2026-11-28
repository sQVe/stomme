use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;

mod agents_block;
mod check;
mod commit;
mod config;
mod fence;
mod full_check;
mod guides;
mod init;
mod layer;
mod ocr_coverage;
mod ocr_rules;
mod ocr_scope;
mod owned;
mod packages;
mod rule;
mod sync;
mod synced_tree;
#[cfg(test)]
mod test_support;

#[cfg(test)]
fn sync_project(root: &std::path::Path) {
    sync::run(root, layer::catalog(), &config::FileRead::filesystem())
        .expect("sync the fixture project");
}

const USAGE: &str = "Usage: stomme <check [--base <ref>]|sync|guides <dir>|init --name <name> --language <language> [--layers <layers>] [--roles <roles>]|ocr-coverage <result-file>|ocr-scope --reviewed <ref> --reviewed-base <ref> --base <ref> --head <ref>>\n\nCheck or sync the project in the current directory, write the guides to a folder, create a project, classify an Open Code Review result, or build the review scope since the last complete OCR review. The ocr-coverage command exits 0 when every selected file was reviewed, 2 for a partial review, and 1 for a failed run or an unusable file.";

#[expect(
    clippy::print_stderr,
    reason = "the usage message and errors are the command's output"
)]
fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();

    match arguments.as_slice() {
        [command] if command == "check" => {
            let pull_request_base = env::var_os("GITHUB_BASE_REF");

            match check::default_reference(pull_request_base) {
                Ok(reference) => run_check(check::TargetBranch::remote_tracking(reference)),
                Err(error) => {
                    eprintln!("stomme: {error:#}");
                    ExitCode::FAILURE
                }
            }
        }
        [command, flag, reference] if command == "check" && flag == "--base" => {
            match reference.to_str() {
                Some(reference) => run_check(check::TargetBranch::git(reference.to_string())),
                None => refuse_non_text(reference),
            }
        }
        [command] if command == "sync" => run_sync(),
        [command, directory] if command == "guides" => run_guides(Path::new(directory)),
        [command, options @ ..] if command == "init" => run_init(options),
        [command, path] if command == "ocr-coverage" => run_ocr_coverage(Path::new(path)),
        [
            command,
            reviewed_flag,
            reviewed,
            reviewed_base_flag,
            reviewed_base,
            base_flag,
            base,
            head_flag,
            head,
        ] if command == "ocr-scope"
            && is_ocr_scope_invocation(reviewed_flag, reviewed_base_flag, base_flag, head_flag) =>
        {
            run_ocr_scope(reviewed, reviewed_base, base, head)
        }
        _ => {
            let non_text = arguments
                .iter()
                .find(|argument| argument.to_str().is_none());

            match non_text {
                Some(argument) => refuse_non_text(argument),
                None => {
                    eprintln!("{USAGE}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn is_ocr_scope_invocation(
    reviewed_flag: &OsStr,
    reviewed_base_flag: &OsStr,
    base_flag: &OsStr,
    head_flag: &OsStr,
) -> bool {
    let flags = [reviewed_flag, reviewed_base_flag, base_flag, head_flag];
    let expected = ["--reviewed", "--reviewed-base", "--base", "--head"];

    flags
        .iter()
        .zip(expected)
        .all(|(flag, expected)| flag.to_str() == Some(expected))
}

#[expect(clippy::print_stderr, reason = "errors are the command's output")]
fn refuse_non_text(argument: &OsStr) -> ExitCode {
    eprintln!("stomme: the argument {argument:?} is not valid UTF-8");
    ExitCode::FAILURE
}

#[expect(
    clippy::print_stderr,
    reason = "problems and errors are the command's output"
)]
fn run_check(target: check::TargetBranch) -> ExitCode {
    let problems = match check_current_directory(target) {
        Ok(problems) => problems,
        Err(error) => {
            eprintln!("stomme: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    for problem in &problems {
        eprintln!("stomme: {problem}");
    }

    if problems.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn check_current_directory(target: check::TargetBranch) -> anyhow::Result<Vec<check::Problem>> {
    let root = env::current_dir().context("Could not read the current directory")?;
    let file = config::FileRead::filesystem();

    check::run(&root, layer::catalog(), &target, &file)
}

#[expect(clippy::print_stderr, reason = "errors are the command's output")]
fn run_sync() -> ExitCode {
    let root = match env::current_dir().context("Could not read the current directory") {
        Ok(root) => root,
        Err(error) => {
            eprintln!("stomme: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    match sync::run(&root, layer::catalog(), &config::FileRead::filesystem()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stomme: {error}");
            ExitCode::FAILURE
        }
    }
}

#[expect(clippy::print_stderr, reason = "errors are the command's output")]
fn run_guides(directory: &Path) -> ExitCode {
    if directory.as_os_str().is_empty() {
        eprintln!("stomme: the guides directory is empty");
        return ExitCode::FAILURE;
    }

    let directory = match absolute_directory(directory) {
        Ok(directory) => directory,
        Err(error) => {
            eprintln!("stomme: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    match guides::run(&directory, layer::catalog()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stomme: {error}");
            ExitCode::FAILURE
        }
    }
}

fn absolute_directory(directory: &Path) -> anyhow::Result<PathBuf> {
    if directory.is_absolute() {
        return Ok(directory.to_path_buf());
    }

    let root = env::current_dir().context("Could not read the current directory")?;

    Ok(root.join(directory))
}

#[expect(clippy::print_stderr, reason = "errors are the command's output")]
fn run_init(arguments: &[OsString]) -> ExitCode {
    let request = match init::parse_arguments(arguments) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("stomme: {error}");
            return ExitCode::FAILURE;
        }
    };

    let root = match env::current_dir().context("Could not read the current directory") {
        Ok(root) => root,
        Err(error) => {
            eprintln!("stomme: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    match init::run(
        &root,
        layer::catalog(),
        &request,
        &config::FileRead::filesystem(),
        init::release_commit_from_environment,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stomme: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_ocr_scope(reviewed: &OsStr, reviewed_base: &OsStr, base: &OsStr, head: &OsStr) -> ExitCode {
    let Some(reviewed) = reviewed.to_str() else {
        return refuse_non_text(reviewed);
    };
    let Some(reviewed_base) = reviewed_base.to_str() else {
        return refuse_non_text(reviewed_base);
    };
    let Some(base) = base.to_str() else {
        return refuse_non_text(base);
    };
    let Some(head) = head.to_str() else {
        return refuse_non_text(head);
    };
    let request = ocr_scope::Request {
        reviewed: reviewed.to_string(),
        reviewed_base: reviewed_base.to_string(),
        base: base.to_string(),
        head: head.to_string(),
    };
    let root = match env::current_dir().context("Could not read the current directory") {
        Ok(root) => root,
        Err(error) => return fail(&format!("stomme: {error:#}")),
    };

    match ocr_scope::run(&root, &request) {
        Ok(ocr_scope::Outcome::Empty) => {
            report_scope("empty", "scope: 0 files, 0 added lines, 0 removed lines")
        }
        Ok(ocr_scope::Outcome::Base {
            commit,
            files,
            added,
            removed,
        }) => {
            let status =
                format!("scope: {files} files, {added} added lines, {removed} removed lines");

            report_scope(&commit, &status)
        }
        Err(error) => fail(&format!("stomme: {error}")),
    }
}

fn report_scope(base: &str, status: &str) -> ExitCode {
    if let Err(error) = write_stdout(&format!("{base}\n")) {
        return fail(&format!("stomme: could not write the scope base: {error}"));
    }

    // The base commit is already on stdout, so a broken status stream must not discard it.
    match write_stderr(&format!("stomme: {status}\n")) {
        Ok(()) | Err(_) => ExitCode::SUCCESS,
    }
}

fn fail(message: &str) -> ExitCode {
    match write_stderr(&format!("{message}\n")) {
        Ok(()) | Err(_) => ExitCode::FAILURE,
    }
}

fn write_stdout(text: &str) -> io::Result<()> {
    let mut stdout = io::stdout().lock();

    stdout.write_all(text.as_bytes())?;
    stdout.flush()
}

fn write_stderr(text: &str) -> io::Result<()> {
    let mut stderr = io::stderr().lock();

    stderr.write_all(text.as_bytes())?;
    stderr.flush()
}

#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "the markdown fragment and status lines are the command's output"
)]
fn run_ocr_coverage(path: &Path) -> ExitCode {
    let label = path.display().to_string();
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!(
                "stomme: could not read {}: {error}",
                ocr_coverage::escaped_text(&label)
            );
            return ExitCode::FAILURE;
        }
    };
    let coverage = match ocr_coverage::parse(&bytes, &label) {
        Ok(coverage) => coverage,
        Err(error) => {
            eprintln!("stomme: {error}");
            return ExitCode::FAILURE;
        }
    };

    print!("{}", coverage.fragment());
    eprintln!("stomme: {}", coverage.status_line());

    for line in coverage.unreviewed_lines() {
        eprintln!("stomme: {line}");
    }

    ExitCode::from(coverage.exit_code())
}
