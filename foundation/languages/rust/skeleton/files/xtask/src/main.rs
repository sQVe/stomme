use std::path::Path;
use std::process::{Command, ExitCode};

struct Step {
    name: &'static str,
    program: &'static str,
    arguments: &'static [&'static str],
}

// Clippy runs before stomme so that `Cargo.lock` exists when `stomme check` reads it.
const STEPS: &[Step] = &[
    Step {
        name: "rustfmt",
        program: "cargo",
        arguments: &["fmt", "--all", "--check"],
    },
    Step {
        name: "clippy",
        program: "cargo",
        arguments: &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    },
    Step {
        name: "test",
        program: "cargo",
        arguments: &["test", "--workspace"],
    },
    Step {
        name: "stomme",
        program: "stomme",
        arguments: &["check"],
    },
];

fn run_step(root: &Path, step: &Step) -> bool {
    Command::new(step.program)
        .args(step.arguments)
        .current_dir(root)
        .status()
        .is_ok_and(|status| status.success())
}

fn failed_steps(root: &Path, steps: &[Step]) -> Vec<&'static str> {
    steps
        .iter()
        .filter(|step| !run_step(root, step))
        .map(|step| step.name)
        .collect()
}

#[expect(
    clippy::print_stderr,
    reason = "the failed steps and the usage message are the command's output"
)]
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    if arguments != ["check"] {
        eprintln!("usage: cargo xtask check");
        return ExitCode::FAILURE;
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the xtask folder sits inside the workspace root");
    let failed = failed_steps(root, STEPS);

    if failed.is_empty() {
        return ExitCode::SUCCESS;
    }

    for name in failed {
        eprintln!("failed: {name}");
    }

    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_step(name: &'static str, script: &'static [&'static str]) -> Step {
        Step {
            name,
            program: "/bin/sh",
            arguments: script,
        }
    }

    #[test]
    fn steps_run_in_order() {
        let names: Vec<&str> = STEPS.iter().map(|step| step.name).collect();

        assert_eq!(names, ["rustfmt", "clippy", "test", "stomme"]);
    }

    #[test]
    fn failed_steps_runs_every_step_after_a_failure() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let steps = [
            shell_step("passes", &["-c", "exit 0"]),
            shell_step("fails", &["-c", "exit 1"]),
            shell_step("later", &["-c", "exit 2"]),
        ];

        assert_eq!(failed_steps(root, &steps), ["fails", "later"]);
    }
}
