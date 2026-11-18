use std::env;
use std::process::ExitCode;

use anyhow::Context as _;

mod agents_block;
mod check;
mod config;
mod full_check;
mod guides;
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

const USAGE: &str = "Usage: stomme <check [--base <ref>]|sync|guides <dir>>\n\nCheck or sync the project in the current directory, or write the guides to a folder.";

#[expect(
    clippy::print_stderr,
    reason = "the usage message is the command's output"
)]
fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();

    match arguments.as_slice() {
        [command] if command == "check" => run_check("origin/main"),
        [command, flag, reference] if command == "check" && flag == "--base" => {
            run_check(reference)
        }
        [command] if command == "sync" => run_sync(),
        [command, directory] if command == "guides" => run_guides(directory),
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
fn run_check(reference: &str) -> ExitCode {
    let problems = match check_current_directory(reference) {
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

fn check_current_directory(reference: &str) -> anyhow::Result<Vec<check::Problem>> {
    let root = env::current_dir().context("Could not read the current directory")?;
    let target = check::TargetBranch::git(reference.to_string());

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
