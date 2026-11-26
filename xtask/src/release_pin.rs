use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use anyhow::Context as _;

use crate::release_check::{self, Outcome};

const USAGE: &str = "\
Usage:
  cargo xtask release-pin --stomme <binary> --version <candidate> --commit <sha> <empty-folder>";

const WORKFLOWS: [(&str, &str); 2] = [
    (
        ".github/workflows/sync-update.yml",
        include_str!("../../foundation/common/skeleton/files/.github/workflows/sync-update.yml"),
    ),
    (
        ".github/workflows/ocr.yml",
        include_str!("../../foundation/common/skeleton/files/.github/workflows/ocr.yml"),
    ),
];
const COMMIT_OVERRIDE: &str = "STOMME_RELEASE_COMMIT";

struct Runner {
    run: fn(binary: &Path, folder: &Path) -> io::Result<Output>,
    read: fn(path: &Path) -> io::Result<String>,
    entries: fn(folder: &Path) -> io::Result<Vec<io::Result<PathBuf>>>,
}

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    binary: PathBuf,
    version: String,
    commit: String,
    folder: PathBuf,
}

#[expect(
    clippy::print_stderr,
    reason = "the usage and error messages are the command's output"
)]
pub(crate) fn run(root: &Path, arguments: &[String]) -> ExitCode {
    let Some(arguments) = parse_arguments(arguments) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };

    let Some(binary) = release_check::resolve_binary(root, &arguments.binary) else {
        return ExitCode::FAILURE;
    };

    let folder = root.join(&arguments.folder);
    let runner = Runner {
        run: run_init,
        read: read_entry,
        entries: read_entries,
    };

    match check_pin(
        &runner,
        &binary,
        &arguments.version,
        &arguments.commit,
        &folder,
    ) {
        Ok(outcome) => release_check::print_report(&folder, &outcome),
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn check_pin(
    runner: &Runner,
    binary: &Path,
    version: &str,
    commit: &str,
    folder: &Path,
) -> anyhow::Result<Outcome> {
    if let Some(failure) = refuse_nonempty_folder(runner, folder) {
        return Ok(failure);
    }

    let init = (runner.run)(binary, folder)
        .with_context(|| format!("Could not run {} in {}", binary.display(), folder.display()))?;

    if !init.status.success() {
        return Ok(Outcome::Failed {
            step: "init",
            output: stderr(&init),
        });
    }

    Ok(check_workflows(runner, version, commit, folder))
}

fn refuse_nonempty_folder(runner: &Runner, folder: &Path) -> Option<Outcome> {
    let entries = match (runner.entries)(folder) {
        Ok(entries) => entries,
        Err(error) => return Some(unreadable_folder(folder, &error)),
    };

    match entries.first() {
        None => None,
        Some(Ok(_)) => Some(nonempty_folder(folder)),
        Some(Err(error)) => Some(unreadable_folder(folder, error)),
    }
}

fn nonempty_folder(folder: &Path) -> Outcome {
    Outcome::Failed {
        step: "folder",
        output: format!(
            "{} must be empty; remove its contents before running the check.",
            folder.display()
        ),
    }
}

fn unreadable_folder(folder: &Path, error: &io::Error) -> Outcome {
    Outcome::Failed {
        step: "folder",
        output: format!("Could not read {}: {error}", folder.display()),
    }
}

fn check_workflows(runner: &Runner, version: &str, commit: &str, folder: &Path) -> Outcome {
    let mut failures = Vec::new();

    for (path, skeleton) in WORKFLOWS {
        let workflow = folder.join(path);
        let text = match (runner.read)(&workflow) {
            Ok(text) => text,
            Err(error) => {
                failures.push(format!("Could not read {}: {error}", workflow.display()));
                continue;
            }
        };

        if let Outcome::Failed { output, .. } = compare_workflow(skeleton, &text, version, commit) {
            failures.push(format!("{} differs.\n{output}", workflow.display()));
        }
    }

    if failures.is_empty() {
        return Outcome::Passed;
    }

    Outcome::Failed {
        step: "pin",
        output: failures.join("\n"),
    }
}

fn compare_workflow(skeleton: &str, text: &str, version: &str, commit: &str) -> Outcome {
    match first_difference(&expected_workflow(skeleton, version, commit), text) {
        None => Outcome::Passed,
        Some(output) => Outcome::Failed {
            step: "pin",
            output,
        },
    }
}

fn expected_workflow(skeleton: &str, version: &str, commit: &str) -> String {
    skeleton
        .replace("{{commit}}", commit)
        .replace("{{version}}", version)
}

fn first_difference(expected: &str, found: &str) -> Option<String> {
    let expected_lines: Vec<&str> = expected.split_inclusive('\n').collect();
    let found_lines: Vec<&str> = found.split_inclusive('\n').collect();

    for (index, (expected_line, found_line)) in expected_lines.iter().zip(&found_lines).enumerate()
    {
        if expected_line != found_line {
            let number = index + 1;
            let expected_text = render_line(expected_line);
            let found_text = render_line(found_line);
            return Some(format!(
                "Line {number} differs.\nExpected:\n  {expected_text}\nFound:\n  {found_text}"
            ));
        }
    }

    if found_lines.len() < expected_lines.len() {
        return Some("The found file ends first.".to_string());
    }

    if expected_lines.len() < found_lines.len() {
        return Some("The expected file ends first.".to_string());
    }

    None
}

fn render_line(line: &str) -> String {
    line.replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn init_command(binary: &Path, folder: &Path) -> Command {
    let mut command = Command::new(binary);

    command
        .args(["init", "--name", "gate", "--language", "rust"])
        .current_dir(folder)
        .env_remove(COMMIT_OVERRIDE);

    command
}

fn run_init(binary: &Path, folder: &Path) -> io::Result<Output> {
    init_command(binary, folder).output()
}

fn read_entry(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

fn read_entries(folder: &Path) -> io::Result<Vec<io::Result<PathBuf>>> {
    let entries = fs::read_dir(folder)?;
    let paths = entries.map(|entry| entry.map(|entry| entry.path()));

    Ok(paths.collect())
}

fn parse_arguments(arguments: &[String]) -> Option<Arguments> {
    match arguments {
        [stomme, binary, version, candidate, commit, sha, folder]
            if stomme == "--stomme" && version == "--version" && commit == "--commit" =>
        {
            if !release_check::is_release_commit(sha) {
                return None;
            }

            Some(Arguments {
                binary: PathBuf::from(binary),
                version: candidate.clone(),
                commit: sha.clone(),
                folder: PathBuf::from(folder),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
