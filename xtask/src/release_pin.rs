use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use anyhow::Context as _;

use crate::release_check::{self, Outcome};

const USAGE: &str = "\
Usage:
  cargo xtask release-pin --stomme <binary> --version <candidate> --commit <sha> <empty-folder>";

const WORKFLOW: &str = ".github/workflows/sync-update.yml";
const PIN_PREFIX: &str = "uses: sQVe/stomme/.github/workflows/sync-update.yml@";
const COMMIT_OVERRIDE: &str = "STOMME_RELEASE_COMMIT";
const COMMIT_LENGTH: usize = 40;

struct Runner {
    run: fn(binary: &Path, folder: &Path) -> io::Result<Output>,
    read: fn(path: &Path) -> io::Result<String>,
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
    let init = (runner.run)(binary, folder)
        .with_context(|| format!("Could not run {} in {}", binary.display(), folder.display()))?;

    if !init.status.success() {
        return Ok(Outcome::Failed {
            step: "init",
            output: stderr(&init),
        });
    }

    let workflow = folder.join(WORKFLOW);
    let text = match (runner.read)(&workflow) {
        Ok(text) => text,
        Err(error) => {
            return Ok(Outcome::Failed {
                step: "pin",
                output: format!("Could not read {}: {error}", workflow.display()),
            });
        }
    };

    Ok(compare_pin(&text, version, commit))
}

fn compare_pin(text: &str, version: &str, commit: &str) -> Outcome {
    let expected = format!("{PIN_PREFIX}{commit} # v{version}");
    let found = pin_lines(text);

    if found.contains(&expected.as_str()) {
        return Outcome::Passed;
    }

    let output = if found.is_empty() {
        format!("Expected:\n  {expected}\nFound no `uses:` line for sync-update.yml.")
    } else {
        let lines: Vec<String> = found.iter().map(|line| format!("  {line}")).collect();
        format!("Expected:\n  {expected}\nFound:\n{}", lines.join("\n"))
    };

    Outcome::Failed {
        step: "pin",
        output,
    }
}

fn pin_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("uses:") && line.contains("sync-update.yml"))
        .collect()
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

fn parse_arguments(arguments: &[String]) -> Option<Arguments> {
    match arguments {
        [stomme, binary, version, candidate, commit, sha, folder]
            if stomme == "--stomme" && version == "--version" && commit == "--commit" =>
        {
            if !is_release_commit(sha) {
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

fn is_release_commit(value: &str) -> bool {
    value.len() == COMMIT_LENGTH && value.bytes().all(is_commit_byte)
}

fn is_commit_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
}

#[cfg(test)]
mod tests;
