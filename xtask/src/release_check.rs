use std::path::{Path, PathBuf};
use std::process::ExitCode;

const COMMIT_LENGTH: usize = 40;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Passed,
    Failed { step: &'static str, output: String },
    NotAdopted { output: String },
}

#[expect(
    clippy::print_stderr,
    reason = "the resolution error is the command's output"
)]
pub(crate) fn resolve_binary(root: &Path, binary: &Path) -> Option<PathBuf> {
    let path = root.join(binary);

    match path.canonicalize() {
        Ok(binary) => Some(binary),
        Err(error) => {
            eprintln!("Could not resolve {}: {error}", path.display());
            None
        }
    }
}

#[expect(clippy::print_stdout, reason = "the report is the command's output")]
pub(crate) fn print_report(folder: &Path, outcome: &Outcome) -> ExitCode {
    print!("{}", report(folder, outcome));

    exit_code(outcome)
}

pub(crate) fn exit_code(outcome: &Outcome) -> ExitCode {
    match outcome {
        Outcome::Passed | Outcome::NotAdopted { .. } => ExitCode::SUCCESS,
        Outcome::Failed { .. } => ExitCode::FAILURE,
    }
}

pub(crate) fn is_release_commit(value: &str) -> bool {
    value.len() == COMMIT_LENGTH && value.bytes().all(is_commit_byte)
}

fn is_commit_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
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
