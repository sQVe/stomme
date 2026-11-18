use std::env;
use std::process::ExitCode;

use anyhow::Context as _;

mod agents_block;
mod check;
mod config;
mod full_check;
mod guides;
mod init;
mod layer;
mod owned;
mod packages;
mod rule;
mod sync;
mod synced_tree;
#[cfg(test)]
mod test_support;

#[cfg(test)]
fn sync_project(root: &std::path::Path) {
    sync::run(root, layer::catalog()).expect("sync the fixture project");
}

const USAGE: &str = "Usage: stomme <check [--base <ref>]|sync|guides <dir>|init --name <name> --language <language> [--layers <layers>] [--roles <roles>]>\n\nCheck or sync the project in the current directory, write the guides to a folder, or create a project.";

#[expect(
    clippy::print_stderr,
    reason = "the usage message and errors are the command's output"
)]
fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();

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
            run_check(check::TargetBranch::git(reference.to_string()))
        }
        [command] if command == "sync" => run_sync(),
        [command, directory] if command == "guides" => run_guides(directory),
        [command, options @ ..] if command == "init" => run_init(options),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
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

    check::run(&root, layer::catalog(), &target)
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

    match sync::run(&root, layer::catalog()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stomme: {error}");
            ExitCode::FAILURE
        }
    }
}

#[expect(clippy::print_stderr, reason = "errors are the command's output")]
fn run_guides(directory: &str) -> ExitCode {
    if directory.is_empty() {
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

    let directory = std::path::PathBuf::from(directory);
    let directory = if directory.is_absolute() {
        directory
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
fn run_init(arguments: &[String]) -> ExitCode {
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

    match init::run(&root, layer::catalog(), &request) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stomme: {error}");
            ExitCode::FAILURE
        }
    }
}
