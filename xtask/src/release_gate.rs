use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use anyhow::Context as _;
use thiserror::Error;

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
    inspect: fn(path: &Path) -> io::Result<fs::Metadata>,
    read: fn(path: &Path) -> io::Result<String>,
}

#[derive(Debug, Error)]
enum ConfigError {
    #[error("Could not inspect {}", path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} is a symbolic link; use a regular file", path.display())]
    SymbolicLink { path: PathBuf },
    #[error("{} is not a regular file", path.display())]
    NotRegularFile { path: PathBuf },
    #[error("Could not read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
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
    let runner = Runner {
        run: run_command,
        inspect: inspect_entry,
        read: read_entry,
    };

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

    if !is_adopted(runner, &config)? {
        let check = run_tool(runner, binary, folder, &["check", "--base", "HEAD"])?;
        return Ok(Outcome::NotAdopted {
            output: stderr(&check),
        });
    }

    rewrite_pinned_version(runner, &config, folder, version)?;

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

fn is_adopted(runner: &Runner, config: &Path) -> Result<bool, ConfigError> {
    let metadata = match (runner.inspect)(config) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(ConfigError::Inspect {
                path: config.to_path_buf(),
                source,
            });
        }
    };

    if metadata.file_type().is_symlink() {
        return Err(ConfigError::SymbolicLink {
            path: config.to_path_buf(),
        });
    }

    if !metadata.file_type().is_file() {
        return Err(ConfigError::NotRegularFile {
            path: config.to_path_buf(),
        });
    }

    Ok(true)
}

fn inspect_entry(path: &Path) -> io::Result<fs::Metadata> {
    fs::symlink_metadata(path)
}

fn read_entry(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
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

fn rewrite_pinned_version(
    runner: &Runner,
    config: &Path,
    folder: &Path,
    version: &str,
) -> anyhow::Result<()> {
    let text = (runner.read)(config).map_err(|source| ConfigError::Read {
        path: config.to_path_buf(),
        source,
    })?;

    let rewritten = rewrite_top_level_version(&text, folder, version)?;

    fs::write(config, rewritten).with_context(|| format!("Could not write {}", config.display()))
}

fn rewrite_top_level_version(text: &str, folder: &Path, version: &str) -> anyhow::Result<String> {
    let document = toml_edit::Document::parse(text)
        .with_context(|| format!("stomme.toml in {} does not parse as TOML", folder.display()))?;

    let Some(item) = document.get("version") else {
        anyhow::bail!(
            "stomme.toml in {} has no top-level version line",
            folder.display()
        );
    };

    let Some(value) = item.as_value().filter(|value| value.is_str()) else {
        anyhow::bail!(
            "stomme.toml in {} has a version that is not a string",
            folder.display()
        );
    };

    let span = value.span().expect("a parsed value has a source span");
    let replacement = toml_edit::Value::from(version).to_string();

    Ok(format!(
        "{}{replacement}{}",
        &text[..span.start],
        &text[span.end..]
    ))
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
