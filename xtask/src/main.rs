mod check;
mod mutation;
mod release_gate;
#[cfg(test)]
mod renovate_preset;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod workflow;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
Usage:
  cargo xtask check [tidy|build]
  cargo xtask mutation changed <base> [--max-mutants N]
  cargo xtask mutation audit <file>... [--max-mutants N]
  cargo xtask release-gate --stomme <binary> --version <candidate> <project-folder>";

fn workspace_root() -> PathBuf {
    let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    manifest_directory
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}

#[expect(
    clippy::print_stderr,
    reason = "the usage message is the command's output"
)]
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();

    match arguments.split_first() {
        Some((task, rest)) if task == "check" => check::run(&root, rest),
        Some((task, rest)) if task == "mutation" => mutation::run(&root, rest),
        Some((task, rest)) if task == "release-gate" => release_gate::run(&root, rest),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
