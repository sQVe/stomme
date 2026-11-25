use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use super::*;
use crate::git_isolation::isolate_git;
use crate::test_support::temporary_directory;

const TOKEN_ACTION: &str = "actions/create-github-app-token@64ab";
const RELEASE_GATE: &str = include_str!("../../../.github/workflows/release-gate.yml");
const SYNC_UPDATE: &str = include_str!("../../../.github/workflows/sync-update.yml");

fn token_step(action: &str, permissions: &[&str]) -> String {
    let mut step = format!("      - uses: {action}\n        with:\n          app-id: app\n");

    for permission in permissions {
        step.push_str("          ");
        step.push_str(permission);
        step.push('\n');
    }

    step
}

#[test]
fn permission_problems_decides_from_permission_lines() {
    let cases: [(&str, String, bool); 10] = [
        (
            "contents read",
            token_step(TOKEN_ACTION, &["permission-contents: read"]),
            true,
        ),
        ("no permission input", token_step(TOKEN_ACTION, &[]), false),
        (
            "pull requests write",
            token_step(
                TOKEN_ACTION,
                &[
                    "permission-contents: read",
                    "permission-pull-requests: write",
                ],
            ),
            false,
        ),
        (
            "contents write",
            token_step(TOKEN_ACTION, &["permission-contents: write"]),
            false,
        ),
        (
            "quoted read",
            token_step(TOKEN_ACTION, &["permission-contents: 'read'"]),
            true,
        ),
        (
            "trailing comment",
            token_step(TOKEN_ACTION, &["permission-contents: read # why"]),
            true,
        ),
        (
            "quoted key",
            token_step(TOKEN_ACTION, &["\"permission-contents\": read"]),
            true,
        ),
        (
            "quoted action without permission",
            token_step("\"actions/create-github-app-token@64ab\"", &[]),
            false,
        ),
        (
            "two token steps with one permission",
            format!(
                "{}{}",
                token_step(TOKEN_ACTION, &["permission-contents: read"]),
                token_step(TOKEN_ACTION, &[])
            ),
            false,
        ),
        (
            "expression value",
            token_step(TOKEN_ACTION, &["permission-contents: ${{ x }}"]),
            false,
        ),
    ];

    for (name, workflow, accepted) in cases {
        let problems = permission_problems(&workflow);

        assert_eq!(problems.is_empty(), accepted, "{name}: {problems:?}");
    }
}

#[test]
fn sync_update_declares_an_optional_guides_directory_input() {
    let entry = input_entry(guides_input(SYNC_UPDATE), "guides-directory");

    assert_eq!(
        entry,
        vec![
            "      guides-directory:",
            "        required: false",
            "        type: string",
        ]
    );
}

#[test]
fn sync_update_writes_the_guides_before_the_commit() {
    let sync = line_index(SYNC_UPDATE, "name: Sync stomme files");
    let guides = line_index(SYNC_UPDATE, "name: Write the guides");
    let commit = line_index(SYNC_UPDATE, "name: Commit and push");

    assert!(sync < guides && guides < commit);

    let step = workflow_step(&workflow_steps(SYNC_UPDATE), "Write the guides");

    assert!(step.contains(
        "\n        if: steps.check.outputs.run == 'true' && inputs['guides-directory'] != ''\n"
    ));
    assert!(step.contains(
        "\n        env:\n          GUIDES_DIRECTORY: ${{ inputs['guides-directory'] }}\n"
    ));
    assert!(step.ends_with("\n        run: stomme guides \"$GUIDES_DIRECTORY\""));
}

#[test]
fn sync_update_checks_the_guides_directory_before_writing() {
    let validation = line_index(SYNC_UPDATE, "name: Check the guides directory");
    let guides = line_index(SYNC_UPDATE, "name: Write the guides");

    assert!(validation < guides);

    let step = workflow_step(&workflow_steps(SYNC_UPDATE), "Check the guides directory");

    assert!(step.contains(
        "\n        if: steps.check.outputs.run == 'true' && inputs['guides-directory'] != ''\n"
    ));
    assert!(step.contains(
        "\n        env:\n          GUIDES_DIRECTORY: ${{ inputs['guides-directory'] }}\n"
    ));
    assert!(step.contains("\n        run: |\n"));
    assert!(step.contains("\n          root=$(realpath -m -- \"$PWD\")\n"));
    assert!(step.contains("\n          destination=$(realpath -m -- \"$GUIDES_DIRECTORY\")\n"));
    assert!(step.contains("\n            \"$root\"/*) ;;\n"));
    assert!(step.contains("\n            \"$root/.git\" | \"$root/.git\"/*)\n"));
    assert!(step.contains("\n            \"$root/docs/stomme\" | \"$root/docs/stomme\"/*)\n"));
    assert!(step.contains("\n          probe=$destination\n"));
    assert!(step.contains("\n          while [ \"$probe\" != \"$root\" ]; do\n"));
    assert!(step.contains(
        "\n            if [ -e \"$probe/.git\" ] || git ls-files --stage -- \"$probe\" | grep -q '^160000'; then\n"
    ));
    assert!(
        step.contains("::error::The guides directory must not be inside another Git repository:")
    );
    assert!(step.contains("\n            probe=$(dirname \"$probe\")\n"));
    assert!(step.contains("\n          done\n"));
    assert!(step.contains(
        "\n          status=0\n          git check-ignore -q -- \"$destination\" || status=$?\n\n          if [ \"$status\" -eq 1 ]; then\n            status=0\n            git check-ignore -q -- \"$destination/\" || status=$?\n          fi\n"
    ));
    assert!(step.contains("\n          if [ \"$status\" -eq 0 ]; then\n"));
    assert!(step.contains("\n          if [ \"$status\" -ne 1 ]; then\n"));
    assert!(step.contains("::error::The guides directory must be a folder inside the repository:"));
    assert!(step.contains("::error::The guides directory must not be inside the Git folder:"));
    assert!(step.contains(
        "::error::The guides directory must not be the project's synced guides folder or a folder inside it:"
    ));
    assert!(step.contains(
        "::error::Git ignores the guides directory, so the sync cannot commit the guides:"
    ));
    assert!(step.contains("::error::Git could not check the guides directory:"));
    assert_eq!(step.matches("exit 1").count(), 6);
}

#[test]
fn sync_update_checks_the_written_guides_before_the_commit() {
    let guides = line_index(SYNC_UPDATE, "name: Write the guides");
    let check = line_index(SYNC_UPDATE, "name: Check the written guides");
    let commit = line_index(SYNC_UPDATE, "name: Commit and push");

    assert!(guides < check && check < commit);

    let step = workflow_step(&workflow_steps(SYNC_UPDATE), "Check the written guides");

    assert!(step.contains(
        "\n        if: steps.check.outputs.run == 'true' && inputs['guides-directory'] != ''\n"
    ));
    assert!(step.contains(
        "\n        env:\n          GUIDES_DIRECTORY: ${{ inputs['guides-directory'] }}\n"
    ));
}

#[test]
fn sync_update_fails_when_git_ignores_a_written_guide() {
    let cases = [
        ("an ignored guide file", Some("/vendor/stomme/shared/\n"), 1),
        ("no ignored guide file", None, 0),
    ];

    for (name, ignore_rule, expected) in cases {
        let repository = temporary_directory();
        let root = repository.path();

        write_file(root, ".gitignore", ignore_rule.unwrap_or(""));
        write_file(root, "vendor/stomme/coding.md", "a\n");
        write_file(root, "vendor/stomme/shared/coding.md", "b\n");

        let step = workflow_step(&workflow_steps(SYNC_UPDATE), "Check the written guides");
        let output = run_step(step, root);
        let code = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(code, Some(expected), "{name}: {stdout}{stderr}");

        if expected == 0 {
            continue;
        }

        assert!(
            stdout.contains("vendor/stomme/shared/coding.md"),
            "{name}: {stdout}{stderr}"
        );
        assert!(
            stdout.contains("::error::Git ignores files inside the guides directory"),
            "{name}: {stdout}{stderr}"
        );
    }
}

#[test]
fn release_gate_token_steps_request_read_only_contents() {
    let token_steps = RELEASE_GATE
        .lines()
        .filter(|line| line.contains("actions/create-github-app-token"))
        .count();

    assert!(token_steps > 0, "the release gate creates an app token");
    assert_eq!(permission_problems(RELEASE_GATE), Vec::<String>::new());
}

fn write_file(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    let parent = path.parent().expect("the file has a parent folder");

    fs::create_dir_all(parent).expect("create the file's folder");
    fs::write(&path, contents).expect("write the file");
}

fn run_step(step: &str, directory: &Path) -> Output {
    let script = step
        .split_once("        run: |\n")
        .unwrap_or_else(|| panic!("the step runs a script: {step}"))
        .1;
    let script = script
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");

    let init = isolated_command("git", directory)
        .args(["init", "--quiet", "."])
        .output()
        .expect("run git init");

    assert!(init.status.success(), "git init failed: {init:?}");

    isolated_command("bash", directory)
        .arg("-e")
        .arg("-c")
        .arg(script)
        .env("GUIDES_DIRECTORY", "vendor/stomme")
        .output()
        .expect("run the workflow step")
}

fn isolated_command(program: &str, directory: &Path) -> Command {
    let mut command = Command::new(program);

    command
        .current_dir(directory)
        .env("HOME", directory)
        .env("XDG_CONFIG_HOME", directory);
    isolate_git(&mut command);

    command
}

fn workflow_steps(workflow: &str) -> Vec<&str> {
    let mut steps = Vec::new();
    let mut remaining = workflow;

    while let Some(start) = remaining.find("\n      - ") {
        let step = &remaining[start + 1..];
        let end = step.find("\n      - ").unwrap_or(step.len());
        steps.push(&step[..end]);
        remaining = &step[end..];
    }

    steps
}

fn workflow_step<'a>(steps: &[&'a str], name: &str) -> &'a str {
    let header = format!("      - name: {name}\n");

    steps
        .iter()
        .copied()
        .find(|step| step.starts_with(header.as_str()))
        .unwrap_or_else(|| panic!("the workflow has a step named {name}"))
}

fn guides_input(workflow: &str) -> &str {
    let trigger = workflow
        .split_once("  workflow_call:\n")
        .expect("the workflow declares a reusable workflow trigger")
        .1;
    let inputs = trigger
        .split_once("    inputs:\n")
        .expect("the trigger declares inputs")
        .1;

    inputs
        .split_once("\n    secrets:")
        .expect("the inputs end at the secrets block")
        .0
}

fn input_entry<'a>(inputs: &'a str, name: &str) -> Vec<&'a str> {
    let header = format!("      {name}:");
    let mut lines = inputs.lines();
    let mut entry = Vec::new();

    while let Some(line) = lines.next() {
        if line != header {
            continue;
        }

        entry.push(line);

        for line in lines.by_ref() {
            if !line.starts_with("        ") {
                break;
            }

            entry.push(line);
        }

        break;
    }

    entry
}

fn line_index(workflow: &str, needle: &str) -> usize {
    workflow
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle} is in the workflow"))
}
