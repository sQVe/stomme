use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

use anyhow::Context as _;

mod agents_block;
mod check;
mod config;
mod full_check;
mod guides;
mod init;
mod layer;
mod ocr_coverage;
mod ocr_rules;
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

const USAGE: &str = "Usage: stomme <check [--base <ref>]|sync|guides <dir>|init --name <name> --language <language> [--layers <layers>] [--roles <roles>]|ocr-coverage <result-file>>\n\nCheck or sync the project in the current directory, write the guides to a folder, create a project, or classify an Open Code Review result. The ocr-coverage command exits 0 when every selected file was reviewed, 2 for a partial review, and 1 for a failed run or an unusable file.";

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
                None => refuse_non_text("the `--base` reference"),
            }
        }
        [command] if command == "sync" => run_sync(),
        [command, directory] if command == "guides" => run_guides(Path::new(directory)),
        [command, options @ ..] if command == "init" => run_init(options),
        [command, path] if command == "ocr-coverage" => run_ocr_coverage(Path::new(path)),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[expect(clippy::print_stderr, reason = "errors are the command's output")]
fn refuse_non_text(argument: &str) -> ExitCode {
    eprintln!("stomme: {argument} is not valid UTF-8");
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

    let root = match env::current_dir().context("Could not read the current directory") {
        Ok(root) => root,
        Err(error) => {
            eprintln!("stomme: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    let directory = if directory.is_absolute() {
        directory.to_path_buf()
    } else {
        root.join(directory)
    };

    match guides::run(&directory, layer::catalog()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stomme: {error}");
            ExitCode::FAILURE
        }
    }
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
            eprintln!("stomme: could not read {label}: {error}");
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
