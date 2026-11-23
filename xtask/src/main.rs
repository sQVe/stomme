mod check;
mod mutation;
mod ocr_smoke;
mod release_check;
mod release_gate;
mod release_pin;
#[cfg(test)]
mod renovate_preset;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod workflow;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
Usage:
  cargo xtask check [tidy|build]
  cargo xtask mutation changed <base> [--max-mutants N]
  cargo xtask mutation audit <file>... [--max-mutants N]
  cargo xtask ocr-smoke
  cargo xtask release-gate --stomme <binary> --version <candidate> <project-folder>
  cargo xtask release-pin --stomme <binary> --version <candidate> --commit <sha> <empty-folder>";

fn workspace_root() -> PathBuf {
    let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    manifest_directory
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}

#[derive(Debug, PartialEq, Eq)]
enum Task {
    Check,
    Mutation,
    OcrSmoke,
    ReleaseGate,
    ReleasePin,
}

impl Task {
    fn run(self, root: &Path, arguments: &[String]) -> ExitCode {
        let rest = arguments.get(1..).unwrap_or_default();

        match self {
            Task::Check => check::run(root, rest),
            Task::Mutation => mutation::run(root, rest),
            Task::OcrSmoke => ocr_smoke::run(root, rest),
            Task::ReleaseGate => release_gate::run(root, rest),
            Task::ReleasePin => release_pin::run(root, rest),
        }
    }
}

fn select_task(arguments: &[String]) -> Option<Task> {
    match arguments.first()?.as_str() {
        "check" => Some(Task::Check),
        "mutation" => Some(Task::Mutation),
        "ocr-smoke" => Some(Task::OcrSmoke),
        "release-gate" => Some(Task::ReleaseGate),
        "release-pin" => Some(Task::ReleasePin),
        _ => None,
    }
}

#[expect(
    clippy::print_stderr,
    reason = "the usage message is the command's output"
)]
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();

    match select_task(&arguments) {
        Some(task) => task.run(&root, &arguments),
        None => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::strings;

    #[test]
    fn select_task_routes_each_task() {
        let cases = [
            ("check", Some(Task::Check)),
            ("mutation", Some(Task::Mutation)),
            ("ocr-smoke", Some(Task::OcrSmoke)),
            ("release-gate", Some(Task::ReleaseGate)),
            ("release-pin", Some(Task::ReleasePin)),
            ("unknown", None),
        ];

        for (name, expected) in cases {
            assert_eq!(select_task(&strings(&[name])), expected, "{name}");
        }
    }

    #[test]
    fn select_task_rejects_no_task() {
        assert_eq!(select_task(&[]), None);
    }
}
