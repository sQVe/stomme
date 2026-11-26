use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use anyhow::Context as _;
use thiserror::Error;

use crate::release_check::{self, Outcome};

const USAGE: &str = "\
Usage:
  cargo xtask release-gate --stomme <binary> --version <candidate> --commit <sha> <project-folder>";

const WORKFLOWS_PREFIX: &str = "sqve/stomme/.github/workflows/";
const OWNER: &str = "sQVe/stomme";

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
        run: run_command,
        inspect: inspect_entry,
        read: read_entry,
    };

    match gate(
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

fn gate(
    runner: &Runner,
    binary: &Path,
    version: &str,
    commit: &str,
    folder: &Path,
) -> anyhow::Result<Outcome> {
    let config = folder.join("stomme.toml");

    if !is_adopted(runner, &config)? {
        let check = run_tool(runner, binary, folder, &["check", "--base", "HEAD"])?;
        return Ok(Outcome::NotAdopted {
            output: stderr(&check),
        });
    }

    rewrite_pinned_version(runner, &config, folder, version)?;
    rewrite_workflow_pins(folder, version, commit)?;

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

fn rewrite_workflow_pins(folder: &Path, version: &str, commit: &str) -> anyhow::Result<()> {
    for path in workflow_paths(folder)? {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Could not read {}", path.display()))?;
        let rewritten = rewrite_workflow_calls(&text, version, commit);

        if rewritten == text {
            continue;
        }

        fs::write(&path, rewritten)
            .with_context(|| format!("Could not write {}", path.display()))?;
    }

    Ok(())
}

fn workflow_paths(folder: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let github = folder.join(".github");

    if is_symlink(&github) || !github.is_dir() {
        return Ok(Vec::new());
    }

    let workflows = github.join("workflows");

    if is_symlink(&workflows) || !workflows.is_dir() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&workflows)
        .with_context(|| format!("Could not read {}", workflows.display()))?;
    let mut paths = Vec::new();

    for entry in entries {
        let entry =
            entry.with_context(|| format!("Could not read an entry of {}", workflows.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("Could not inspect {}", entry.path().display()))?;

        if !file_type.is_file() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();

        if is_workflow_name(&name) {
            paths.push(entry.path());
        }
    }

    paths.sort();

    Ok(paths)
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
}

fn is_workflow_name(name: &str) -> bool {
    name.ends_with(".yml") || name.ends_with(".yaml")
}

fn rewrite_workflow_calls(text: &str, version: &str, commit: &str) -> String {
    let mut rewritten = String::with_capacity(text.len());

    for line in text.split_inclusive('\n') {
        let (body, ending) = split_line_ending(line);

        match rewrite_workflow_call(body, version, commit) {
            Some(call) => rewritten.push_str(&call),
            None => rewritten.push_str(body),
        }

        rewritten.push_str(ending);
    }

    rewritten
}

fn split_line_ending(line: &str) -> (&str, &str) {
    let Some(body) = line.strip_suffix('\n') else {
        return (line, "");
    };

    match body.strip_suffix('\r') {
        Some(body) => (body, "\r\n"),
        None => (body, "\n"),
    }
}

fn rewrite_workflow_call(line: &str, version: &str, commit: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let (dash, trimmed) = match trimmed.strip_prefix('-') {
        Some(rest) if rest.starts_with(char::is_whitespace) => ("- ", rest.trim_start()),
        _ => ("", trimmed),
    };
    let rest = trimmed.strip_prefix("uses:")?;
    let called = called_workflow(rest.trim());

    if !called.to_ascii_lowercase().starts_with(WORKFLOWS_PREFIX) {
        return None;
    }

    let workflow = called.rsplit('/').next().unwrap_or(called);
    let call = format!("{OWNER}/.github/workflows/{workflow}@{commit} # v{version}");

    Some(format!("{indent}{dash}uses: {call}"))
}

fn called_workflow(value: &str) -> &str {
    let value = unquoted(value);
    let value = value
        .split_once(char::is_whitespace)
        .map_or(value, |(value, _)| value);
    let value = value.trim_end_matches(['"', '\'']);

    value.split_once('@').map_or(value, |(called, _)| called)
}

fn unquoted(value: &str) -> &str {
    for quote in ['"', '\''] {
        let Some(rest) = value.strip_prefix(quote) else {
            continue;
        };

        return rest.strip_suffix(quote).unwrap_or(rest);
    }

    value
}

#[cfg(test)]
mod tests;
