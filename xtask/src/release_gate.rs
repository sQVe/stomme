use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use anyhow::Context as _;

const USAGE: &str = "\
Usage:
  cargo xtask release-gate --stomme <binary> --version <candidate> <project-folder>";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Passed,
    Failed { step: &'static str, output: String },
    NotAdopted { output: String },
}

struct Runner {
    run: fn(binary: &Path, folder: &Path, arguments: &[&str]) -> io::Result<Output>,
}

#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    binary: PathBuf,
    version: String,
    folder: PathBuf,
}

#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "the report, usage, and errors are the command's output"
)]
pub(crate) fn run(root: &Path, arguments: &[String]) -> ExitCode {
    let Some(arguments) = parse_arguments(arguments) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };

    let binary_path = root.join(&arguments.binary);
    let binary = match binary_path.canonicalize() {
        Ok(binary) => binary,
        Err(error) => {
            eprintln!("Could not resolve {}: {error}", binary_path.display());
            return ExitCode::FAILURE;
        }
    };

    let folder = root.join(&arguments.folder);
    let runner = Runner { run: run_command };

    match gate(&runner, &binary, &arguments.version, &folder) {
        Ok(outcome) => {
            print!("{}", report(&folder, &outcome));
            exit_code(&outcome)
        }
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn gate(runner: &Runner, binary: &Path, version: &str, folder: &Path) -> anyhow::Result<Outcome> {
    let config = folder.join("stomme.toml");

    if !config.exists() {
        let check = run_tool(runner, binary, folder, &["check", "--base", "HEAD"])?;
        return Ok(Outcome::NotAdopted {
            output: stderr(&check),
        });
    }

    rewrite_pinned_version(&config, folder, version)?;

    let sync = run_tool(runner, binary, folder, &["sync"])?;

    if !sync.status.success() {
        return Ok(Outcome::Failed {
            step: "sync",
            output: stderr(&sync),
        });
    }

    let check = run_tool(runner, binary, folder, &["check", "--base", "HEAD"])?;

    if !check.status.success() {
        return Ok(Outcome::Failed {
            step: "check",
            output: stderr(&check),
        });
    }

    Ok(Outcome::Passed)
}

fn exit_code(outcome: &Outcome) -> ExitCode {
    match outcome {
        Outcome::Passed | Outcome::NotAdopted { .. } => ExitCode::SUCCESS,
        Outcome::Failed { .. } => ExitCode::FAILURE,
    }
}

fn parse_arguments(arguments: &[String]) -> Option<Arguments> {
    match arguments {
        [stomme, binary, version, candidate, folder]
            if stomme == "--stomme" && version == "--version" =>
        {
            Some(Arguments {
                binary: PathBuf::from(binary),
                version: candidate.clone(),
                folder: PathBuf::from(folder),
            })
        }
        _ => None,
    }
}

fn run_command(binary: &Path, folder: &Path, arguments: &[&str]) -> io::Result<Output> {
    Command::new(binary)
        .args(arguments)
        .current_dir(folder)
        .output()
}

fn run_tool(
    runner: &Runner,
    binary: &Path,
    folder: &Path,
    arguments: &[&str],
) -> anyhow::Result<Output> {
    (runner.run)(binary, folder, arguments)
        .with_context(|| format!("Could not run {} in {}", binary.display(), folder.display()))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn rewrite_pinned_version(config: &Path, folder: &Path, version: &str) -> anyhow::Result<()> {
    let text = fs::read_to_string(config)
        .with_context(|| format!("Could not read {}", config.display()))?;

    let Some(rewritten) = rewrite_top_level_version(&text, version) else {
        anyhow::bail!(
            "stomme.toml in {} has no top-level version line",
            folder.display()
        );
    };

    fs::write(config, rewritten).with_context(|| format!("Could not write {}", config.display()))
}

fn rewrite_top_level_version(text: &str, version: &str) -> Option<String> {
    let mut rewritten = String::new();
    let mut replaced = false;
    let mut in_table = false;

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();

        if trimmed.starts_with('[') {
            in_table = true;
        }

        let is_candidate = !replaced && !in_table && is_version_line(trimmed);

        if is_candidate {
            let indent = &line[..line.len() - trimmed.len()];
            let ending = if line.ends_with('\n') { "\n" } else { "" };

            rewritten.push_str(&format!("{indent}version = \"{version}\"{ending}"));
            replaced = true;
        } else {
            rewritten.push_str(line);
        }
    }

    replaced.then_some(rewritten)
}

fn is_version_line(line: &str) -> bool {
    let Some((key, value)) = line.split_once('=') else {
        return false;
    };

    let Some(quoted) = value.trim().strip_prefix('"') else {
        return false;
    };

    key.trim() == "version" && quoted.strip_suffix('"').is_some()
}

fn report(folder: &Path, outcome: &Outcome) -> String {
    let name = folder.file_name().map_or_else(
        || folder.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );

    let mut text = format!("### {name}\n");

    match outcome {
        Outcome::Passed => text.push_str("Passed\n"),
        Outcome::Failed { step, output } => {
            text.push_str(&format!("Failed at {step}\n"));
            text.push_str(&output_block(output));
        }
        Outcome::NotAdopted { output } => {
            text.push_str("Not adopted; reported only\n");
            text.push_str(&output_block(output));
        }
    }

    text
}

fn output_block(output: &str) -> String {
    let body = output.trim_end_matches('\n');

    format!("```text\n{body}\n```\n")
}

#[cfg(test)]
mod tests;
