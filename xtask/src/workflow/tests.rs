use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;
use thiserror::Error;

use serde_json::{Value, json};

use super::*;
use crate::git_isolation::isolate_git;
use crate::test_support::temporary_directory;

const TOKEN_ACTION: &str = "actions/create-github-app-token@64ab";
const SYNC_GUARD: &str = "Check the trigger";
const CI: &str = include_str!("../../../.github/workflows/ci.yml");
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
    let commit = line_index(SYNC_UPDATE, "name: Commit the sync");

    assert!(sync < guides && guides < commit);

    let step = workflow_step(&job_steps(SYNC_UPDATE, "sync"), "Write the guides");

    assert!(step.contains(
        "\n        if: steps.guard.outputs.run == 'true' && steps.check.outputs.run == 'true' && inputs['guides-directory'] != ''\n"
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

    let step = workflow_step(
        &job_steps(SYNC_UPDATE, "sync"),
        "Check the guides directory",
    );

    assert!(step.contains(
        "\n        if: steps.guard.outputs.run == 'true' && steps.check.outputs.run == 'true' && inputs['guides-directory'] != ''\n"
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
    let commit = line_index(SYNC_UPDATE, "name: Commit the sync");

    assert!(guides < check && check < commit);

    let step = workflow_step(&job_steps(SYNC_UPDATE, "sync"), "Check the written guides");

    assert!(step.contains(
        "\n        if: steps.guard.outputs.run == 'true' && steps.check.outputs.run == 'true' && inputs['guides-directory'] != ''\n"
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

        let step = workflow_step(&job_steps(SYNC_UPDATE, "sync"), "Check the written guides");
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

#[test]
fn release_workflow_starts_only_on_v_tags() {
    let directory = crate::workspace_root().join(".github/workflows");

    assert_eq!(release_tag_problems(&directory), Vec::<String>::new());
}

#[test]
fn release_tag_guard_reads_a_yaml_workflow() {
    let directory = release_workflows(&[("extra.yaml", "on: push\n")]);
    let problems = release_tag_problems(directory.path());

    assert_eq!(
        problems,
        [format!(
            "extra.yaml can start on a tag push matching '{ANY_TAG}'"
        )]
    );
}

#[test]
fn release_tag_guard_reports_a_leftover_release_workflow() {
    let directory = release_workflows(&[("release.yml", "on:\n  pull_request:\n")]);
    let problems = release_tag_problems(directory.path());

    assert_eq!(
        problems,
        ["release.yml still exists; the Release workflow is v-release.yml"]
    );
}

#[test]
fn tag_push_patterns_reads_each_trigger_form() {
    let cases: [(&str, &str, Vec<&str>); 20] = [
        (
            "a block list",
            "on:\n  push:\n    tags:\n      - 'v1.0.0'\n",
            vec!["v1.0.0"],
        ),
        (
            "a block list at the key's indentation",
            "on:\n  push:\n    tags:\n    - 'v1.0.0'\n",
            vec!["v1.0.0"],
        ),
        (
            "an inline list",
            "on:\n  push:\n    tags: ['v1.0.0', \"v2.0.0\"]\n",
            vec!["v1.0.0", "v2.0.0"],
        ),
        (
            "a single tag value",
            "on:\n  push:\n    tags: v1.0.0\n",
            vec!["v1.0.0"],
        ),
        ("a single-line push", "on: push\n", vec![ANY_TAG]),
        (
            "a commented single-line push",
            "on: push # why\n",
            vec![ANY_TAG],
        ),
        ("a quoted single-line push", "on: 'push'\n", vec![ANY_TAG]),
        (
            "a flow list of events with a push",
            "on: [push, pull_request]\n",
            vec![ANY_TAG],
        ),
        (
            "a flow list of events without a push",
            "on: [pull_request]\n",
            Vec::new(),
        ),
        (
            "a scalar event that is not a push",
            "on: workflow_dispatch\n",
            Vec::new(),
        ),
        ("a push without a filter", "on:\n  push:\n", vec![ANY_TAG]),
        (
            "blank lines and comments",
            "on:\n\n  # why\n  push:\n    # why\n    tags:\n\n      - 'v1.0.0' # why\n",
            vec!["v1.0.0"],
        ),
        ("another event", "on:\n  pull_request:\n", Vec::new()),
        (
            "a branch filter",
            "on:\n  push:\n    branches: [main]\n",
            Vec::new(),
        ),
        (
            "a deeper branch filter",
            "on:\n  push:\n      branches: [main]\n",
            Vec::new(),
        ),
        (
            "a branch exclusion without a tag filter",
            "on:\n  push:\n    branches-ignore: [main]\n",
            Vec::new(),
        ),
        (
            "a path filter alone",
            "on:\n  push:\n    paths: ['src/**']\n",
            vec![ANY_TAG],
        ),
        (
            "a tag exclusion",
            "on:\n  push:\n    tags-ignore: ['v*']\n",
            vec![ANY_TAG],
        ),
        (
            "a deeper tag list",
            "on:\n  push:\n      tags:\n          - 'v1.0.0'\n",
            vec!["v1.0.0"],
        ),
        (
            "a flow mapping of filters",
            "on:\n  push: {tags: ['v1.0.0', 'v2.0.0']}\n",
            vec!["v1.0.0", "v2.0.0"],
        ),
    ];

    for (name, workflow, expected) in cases {
        let patterns = tag_push_patterns(workflow).expect("the guard reads the trigger form");
        let actual: Vec<&str> = patterns.iter().map(String::as_str).collect();

        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn tag_push_patterns_reports_a_trigger_it_cannot_read() {
    let cases = [
        (
            "a flow mapping of events",
            "on: {push: {tags: ['v1.0.0']}}\n",
            TriggerError::Unreadable("on: {push: {tags: ['v1.0.0']}}".to_string()),
        ),
        (
            "an event block sequence",
            "on:\n  - push\n",
            TriggerError::Unreadable("- push".to_string()),
        ),
        (
            "a nested filter mapping",
            "on:\n  push:\n    tags:\n      v1: x\n",
            TriggerError::Unreadable("v1: x".to_string()),
        ),
        (
            "an unknown push filter",
            "on:\n  push:\n    labels: [x]\n",
            TriggerError::Unreadable("labels: [x]".to_string()),
        ),
        (
            "a malformed tag list",
            "on:\n  push:\n    tags: ['v1.0.0'\n",
            TriggerError::Unreadable("tags: ['v1.0.0'".to_string()),
        ),
        ("no trigger", "name: Release\n", TriggerError::NoTrigger),
    ];

    for (name, workflow, expected) in cases {
        let scan = tag_push_patterns(workflow);

        assert_eq!(scan, Err(expected), "{name}");
    }
}

#[test]
fn sync_update_guards_the_trigger_before_creating_the_app_token() {
    let sync = job_steps(SYNC_UPDATE, "sync");
    let guard = workflow_step_index(&sync, SYNC_GUARD);
    let push = job_steps(SYNC_UPDATE, "push");

    assert_eq!(guard, 0, "the guard is the first sync step");
    assert!(
        !sync
            .iter()
            .any(|step| step.contains("actions/create-github-app-token")),
        "the sync job creates the app token"
    );
    assert!(
        push.iter()
            .any(|step| step.contains("actions/create-github-app-token"))
    );

    let step = sync[guard];

    assert!(step.contains("\n        id: guard\n"));
    assert!(step.contains(
        "\n        env:\n          EVENT_NAME: ${{ github.event_name }}\n          REF: ${{ github.ref }}\n          ACTOR_ID: ${{ github.actor_id }}\n"
    ));
    assert!(step.contains("if [ \"$EVENT_NAME\" != \"push\" ]"));
    assert!(step.contains("if [ \"$REF\" != \"refs/heads/renovate/stomme\" ]"));
    assert!(step.contains("if [ \"$ACTOR_ID\" != \"29139614\" ]"));
}

#[test]
fn sync_update_creates_the_app_token_only_in_the_push_job() {
    let jobs = workflow_jobs(SYNC_UPDATE);

    assert!(jobs.iter().any(|(name, _)| *name == "push"));

    for (name, job) in &jobs {
        let creates_token = job.contains("actions/create-github-app-token");
        let references_token = job.contains("steps.token");
        let reads_secrets = job.contains("app-id") || job.contains("app-private-key");

        if *name == "push" {
            assert!(creates_token, "the push job creates the app token");
            continue;
        }

        assert!(!creates_token, "the {name} job creates the app token");
        assert!(!references_token, "the {name} job references the app token");
        assert!(!reads_secrets, "the {name} job references the app secrets");
    }
}

#[test]
fn sync_update_pushes_from_a_trusted_job() {
    let sync = workflow_job(SYNC_UPDATE, "sync");
    let push = workflow_job(SYNC_UPDATE, "push");

    assert!(sync.contains(
        "\n    outputs:\n      changed: ${{ steps.commit.outputs.changed }}\n      commit: ${{ steps.commit.outputs.commit }}\n      artifact: ${{ steps.upload.outputs.artifact-id }}\n"
    ));
    assert!(push.starts_with("  push:\n"));
    assert!(push.contains("\n    needs: sync\n"));
    assert!(push.contains("\n    if: needs.sync.outputs.changed == 'true'\n"));
}

#[test]
fn sync_update_push_job_runs_no_branch_setup() {
    let push = workflow_job(SYNC_UPDATE, "push");

    for marker in [
        "actions/checkout@",
        "actions/download-artifact",
        "jdx/mise-action@",
        "pnpm/action-setup@",
        "actions/setup-node@",
        "oven-sh/setup-bun@",
        "stomme sync",
        "stomme guides",
    ] {
        assert!(!push.contains(marker), "the push job runs {marker}");
    }
}

#[test]
fn sync_update_validates_the_bundle_before_the_token() {
    let steps = job_steps(SYNC_UPDATE, "push");
    let fetch = workflow_step_index(&steps, "Fetch the sync artifact");
    let bundle = workflow_step_index(&steps, "Check the sync bundle");
    let token = step_index_containing(&steps, "actions/create-github-app-token");
    let step = steps[bundle];

    assert!(fetch < bundle, "the fetch runs before the bundle check");
    assert!(
        bundle < token,
        "the bundle check runs before the token step"
    );
    assert!(step.contains("\n          EXPECTED_COMMIT: ${{ needs.sync.outputs.commit }}\n"));
    assert!(step.contains("git bundle verify sync.bundle"));
    assert!(step.contains("git bundle list-heads sync.bundle"));
    assert!(step.contains("git rev-list --count \"$EXPECTED_SHA..$tip\""));
    assert!(step.contains("git rev-list --parents -n 1 \"$tip\""));
    assert!(step.contains("\"$tip $EXPECTED_SHA\""));
}

#[test]
fn sync_update_gates_every_sync_step_on_the_full_condition() {
    let steps = job_steps(SYNC_UPDATE, "sync");
    let guard = "steps.guard.outputs.run == 'true'";
    let checked = "steps.guard.outputs.run == 'true' && steps.check.outputs.run == 'true'";
    let guides = "steps.guard.outputs.run == 'true' && steps.check.outputs.run == 'true' && inputs['guides-directory'] != ''";
    let pnpm = "steps.guard.outputs.run == 'true' && steps.check.outputs.run == 'true' && steps.lockfile.outputs.kind == 'pnpm'";
    let bun = "steps.guard.outputs.run == 'true' && steps.check.outputs.run == 'true' && steps.lockfile.outputs.kind == 'bun'";
    let changed = "steps.guard.outputs.run == 'true' && steps.check.outputs.run == 'true' && steps.commit.outputs.changed == 'true'";
    let cases = [
        ("actions/checkout@", guard),
        ("name: Check the update commit", guard),
        ("jdx/mise-action@", checked),
        ("name: Sync stomme files", checked),
        ("name: Check the guides directory", guides),
        ("name: Write the guides", guides),
        ("name: Check the written guides", guides),
        ("name: Detect the lockfile", checked),
        ("pnpm/action-setup@", pnpm),
        ("actions/setup-node@", pnpm),
        ("name: Refresh the pnpm lockfile", pnpm),
        ("oven-sh/setup-bun@", bun),
        ("name: Refresh the Bun lockfile", bun),
        ("name: Commit the sync", checked),
        ("actions/upload-artifact@", changed),
    ];

    assert_eq!(
        steps.len(),
        cases.len() + 1,
        "every step after the guard is in the table"
    );

    for (marker, condition) in cases {
        let index = step_index_containing(&steps, marker);
        let actual = step_if(steps[index])
            .unwrap_or_else(|| panic!("the step with {marker} has no if condition"));

        assert_eq!(actual, condition, "{marker}");
    }
}

#[test]
fn sync_update_checks_out_without_credentials() {
    let steps = job_steps(SYNC_UPDATE, "sync");
    let step = steps[step_index_containing(&steps, "actions/checkout@")];

    assert!(
        !step.contains("token:"),
        "the checkout passes no token: {step}"
    );
    assert!(
        step.contains("\n          persist-credentials: false"),
        "the checkout persists no credentials: {step}"
    );
}

#[test]
fn sync_update_pushes_with_the_app_token_and_no_hooks() {
    let step = workflow_step(&job_steps(SYNC_UPDATE, "push"), "Push the sync");

    assert!(step.contains("\n          GH_TOKEN: ${{ steps.token.outputs.token }}\n"));
    assert!(step.contains("\n          TIP: ${{ steps.bundle.outputs.tip }}\n"));
    assert!(step.contains("--force-with-lease=\"refs/heads/renovate/stomme:${EXPECTED_SHA}\""));
    assert!(step.contains("core.hooksPath=/dev/null"));
    assert!(step.contains("http.https://github.com/.extraheader=$auth"));
    assert!(step.contains("grep -qE 'non-fast-forward|fetch first|stale info'"));
    assert!(
        step.contains("push conflict on renovate/stomme; the branch moved, so nothing was pushed")
    );
}

#[test]
fn sync_update_fetches_the_artifact_without_extracting_it() {
    let steps = job_steps(SYNC_UPDATE, "push");
    let fetch = workflow_step_index(&steps, "Fetch the sync artifact");
    let bundle = workflow_step_index(&steps, "Check the sync bundle");
    let token = step_index_containing(&steps, "actions/create-github-app-token");
    let fetch_step = steps[fetch];
    let bundle_step = steps[bundle];

    assert!(fetch < bundle && bundle < token);
    assert!(!SYNC_UPDATE.contains("actions/download-artifact"));
    assert!(fetch_step.contains("\n          ARTIFACT_ID: ${{ needs.sync.outputs.artifact }}\n"));
    assert!(fetch_step.contains("directory=$(mktemp -d \"$RUNNER_TEMP/sync.XXXXXX\")"));
    assert!(fetch_step.contains(
        "gh api \"/repos/${REPOSITORY}/actions/artifacts/${ARTIFACT_ID}/zip\" > \"$directory/sync-artifact.zip\""
    ));
    assert!(bundle_step.contains("\n          WORK: ${{ steps.artifact.outputs.directory }}\n"));
    assert!(bundle_step.contains("unzip -p \"$archive\" sync.bundle > \"$WORK/sync.bundle\""));
    assert!(bundle_step.contains("if [ ! -f \"$archive\" ]; then"));
    assert!(bundle_step.contains("if [ ! -f sync.bundle ] || [ ! -s sync.bundle ]; then"));
    assert!(bundle_step.contains("git init --quiet ."));
}

#[test]
fn sync_update_pins_the_upload_artifact_action() {
    assert!(
        SYNC_UPDATE
            .contains("actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2"),
        "the workflow pins the upload action"
    );
    assert!(!SYNC_UPDATE.contains("actions/download-artifact"));
}

#[test]
fn sync_update_reads_only_the_bundle_entry_from_the_artifact() {
    let directory = temporary_directory();
    let root = directory.path();
    let (remote, base, tip, _) = bundle_repository(root);
    let work = root.join("work");
    let bundle = root.join("one.bundle");

    bundle_range(&work, &bundle, &format!("{base}..refs/heads/tip"));

    let contents = fs::read(&bundle).expect("read the bundle");
    let archive = zip_stored(&[
        ("sync.bundle", contents.as_slice()),
        (
            ".git/config",
            b"[user]\n\tname = branch-controlled\n".as_slice(),
        ),
    ]);
    let artifact = root.join("artifact.zip");

    fs::write(&artifact, &archive).expect("write the artifact");

    let bin = fake_gh(root);
    let config = write_git_config(root, &remote);
    let steps = job_steps(SYNC_UPDATE, "push");
    let fetch = workflow_step(&steps, "Fetch the sync artifact");
    let check = workflow_step(&steps, "Check the sync bundle");
    let runner_temp = root.join("runner-temp");

    fs::create_dir(&runner_temp).expect("create the runner temp folder");

    let fetch_output = root.join("fetch-output");

    fs::write(&fetch_output, "").expect("create the step output file");

    let fetched = run_script(
        fetch,
        root,
        &bin,
        &config,
        &[
            ("GH_TOKEN", "token"),
            ("REPOSITORY", "test/repo"),
            ("ARTIFACT_ID", "7"),
            ("RUNNER_TEMP", path_text(&runner_temp)),
            ("FAKE_ARTIFACT", path_text(&artifact)),
            ("GITHUB_OUTPUT", path_text(&fetch_output)),
        ],
    );

    assert_eq!(
        fetched.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&fetched.stderr)
    );

    let written = fs::read_to_string(&fetch_output).expect("read the fetch output");
    let work_directory = written
        .trim()
        .strip_prefix("directory=")
        .expect("the fetch step reports its directory");
    let names: Vec<String> = fs::read_dir(work_directory)
        .expect("list the work directory")
        .map(|entry| {
            entry
                .expect("read an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    assert_eq!(names, ["sync-artifact.zip"]);

    let check_output = root.join("check-output");

    fs::write(&check_output, "").expect("create the step output file");

    let checked = run_script(
        check,
        root,
        &bin,
        &config,
        &[
            ("WORK", work_directory),
            ("GITHUB_TOKEN", "token"),
            ("REPOSITORY", "test/repo"),
            ("EXPECTED_SHA", &base),
            ("EXPECTED_COMMIT", &tip),
            ("GITHUB_OUTPUT", path_text(&check_output)),
        ],
    );

    assert_eq!(
        checked.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&checked.stdout)
    );
    assert_eq!(
        fs::read_to_string(&check_output).expect("read the check output"),
        format!("tip={tip}\n")
    );

    let git_config = fs::read_to_string(Path::new(work_directory).join(".git/config"))
        .expect("read the repository config");

    assert!(!git_config.contains("branch-controlled"), "{git_config}");
}

#[test]
fn sync_update_refuses_an_artifact_without_a_bundle_entry() {
    let directory = temporary_directory();
    let root = directory.path();
    let bin = fake_gh(root);
    let config = write_git_config(root, &root.join("remote.git"));
    let step = workflow_step(&job_steps(SYNC_UPDATE, "push"), "Check the sync bundle");
    let cases = [
        (
            "a missing entry",
            zip_stored(&[("other.txt", b"x".as_slice())]),
        ),
        (
            "a directory entry",
            zip_stored(&[("sync.bundle/", b"".as_slice())]),
        ),
    ];

    for (name, archive) in cases {
        let case = temporary_directory();
        let output = case.path().join("output");

        fs::write(case.path().join("sync-artifact.zip"), &archive).expect("write the artifact");
        fs::write(&output, "").expect("create the step output file");

        let result = run_script(
            step,
            case.path(),
            &bin,
            &config,
            &[
                ("WORK", path_text(case.path())),
                ("GITHUB_TOKEN", "token"),
                ("REPOSITORY", "test/repo"),
                ("EXPECTED_SHA", "0000000000000000000000000000000000000000"),
                (
                    "EXPECTED_COMMIT",
                    "0000000000000000000000000000000000000000",
                ),
                ("GITHUB_OUTPUT", path_text(&output)),
            ],
        );

        assert_eq!(
            result.status.code(),
            Some(1),
            "{name}: {}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert!(
            String::from_utf8_lossy(&result.stdout).contains("sync artifact"),
            "{name}: {}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
}

#[test]
fn sync_update_rejects_a_bundle_that_does_not_match_the_sync() {
    let directory = temporary_directory();
    let root = directory.path();
    let (remote, base, tip, tip2) = bundle_repository(root);
    let work = root.join("work");
    let bin = fake_gh(root);
    let config = write_git_config(root, &remote);
    let step = workflow_step(&job_steps(SYNC_UPDATE, "push"), "Check the sync bundle");
    let one = root.join("one.bundle");
    let two = root.join("two.bundle");

    bundle_range(&work, &one, &format!("{base}..refs/heads/tip"));
    bundle_range(&work, &two, &format!("{base}..refs/heads/tip2"));

    let (matching, written) = run_bundle_check(step, &bin, &config, &one, &base, &tip);

    assert_eq!(
        matching.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&matching.stderr)
    );
    assert_eq!(written, format!("tip={tip}\n"));

    let (mismatch, _) = run_bundle_check(step, &bin, &config, &one, &base, &base);

    assert_eq!(mismatch.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&mismatch.stdout)
            .contains("The sync bundle tip is not the commit the sync job reported."),
        "{}",
        String::from_utf8_lossy(&mismatch.stdout)
    );

    let (two_commits, _) = run_bundle_check(step, &bin, &config, &two, &base, &tip2);

    assert_eq!(two_commits.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&two_commits.stdout)
            .contains("exactly one commit above the expected commit"),
        "{}",
        String::from_utf8_lossy(&two_commits.stdout)
    );

    let parent_root = temporary_directory();
    let (parent_remote, parent_base, parent_tip) = sibling_bundle_repository(parent_root.path());
    let parent_work = parent_root.path().join("work");
    let parent_bundle = parent_root.path().join("parent.bundle");

    bundle_range(
        &parent_work,
        &parent_bundle,
        &format!("{parent_base}..refs/heads/tip"),
    );

    let parent_bin = fake_gh(parent_root.path());
    let parent_config = write_git_config(parent_root.path(), &parent_remote);
    let (wrong_parent, _) = run_bundle_check(
        step,
        &parent_bin,
        &parent_config,
        &parent_bundle,
        &parent_base,
        &parent_tip,
    );

    assert_eq!(wrong_parent.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&wrong_parent.stdout).contains("only parent"),
        "{}",
        String::from_utf8_lossy(&wrong_parent.stdout)
    );
}

#[test]
fn sync_update_push_reports_success_and_a_lease_rejection() {
    let directory = temporary_directory();
    let root = directory.path();
    let (remote, base, tip, tip2) = bundle_repository(root);
    let work = root.join("work");
    let bin = fake_gh(root);
    let config = write_git_config(root, &remote);
    let step = workflow_step(&job_steps(SYNC_UPDATE, "push"), "Push the sync");
    let environment = [
        ("WORK", path_text(&work)),
        ("GH_TOKEN", "token"),
        ("APP_SLUG", "stomme"),
        ("REPOSITORY", "test/repo"),
        ("EXPECTED_SHA", base.as_str()),
        ("TIP", tip.as_str()),
    ];

    let first = run_script(step, &work, &bin, &config, &environment);

    assert_eq!(
        first.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let remote_path = path_text(&remote);
    let pushed = git(
        root,
        &["-C", remote_path, "rev-parse", "refs/heads/renovate/stomme"],
    );

    assert_ne!(pushed, base);
    assert_ne!(pushed, tip);
    assert_eq!(
        git(
            root,
            &["-C", remote_path, "rev-parse", &format!("{pushed}^")]
        ),
        base
    );
    assert_eq!(
        git(
            root,
            &["-C", remote_path, "log", "-1", "--format=%ae", &pushed]
        ),
        "42+stomme[bot]@users.noreply.github.com"
    );
    assert_eq!(
        git(
            root,
            &["-C", remote_path, "log", "-1", "--format=%s", &pushed]
        ),
        "chore: sync stomme files"
    );

    git(
        &work,
        &[
            "push",
            "--quiet",
            "--force",
            remote_path,
            &format!("{tip2}:refs/heads/renovate/stomme"),
        ],
    );

    let second = run_script(step, &work, &bin, &config, &environment);

    assert_eq!(
        second.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(
        String::from_utf8_lossy(&second.stderr)
            .contains("push conflict on renovate/stomme; the branch moved, so nothing was pushed")
    );
    assert_eq!(
        git(
            root,
            &["-C", remote_path, "rev-parse", "refs/heads/renovate/stomme"]
        ),
        tip2,
        "the rejected push left the branch alone"
    );
}

#[test]
fn sync_update_guard_allows_only_a_renovate_push() {
    let matching = run_guard(&[
        ("EVENT_NAME", "push"),
        ("REF", "refs/heads/renovate/stomme"),
        ("ACTOR_ID", "29139614"),
    ]);

    assert_eq!(matching, "run=true\n");

    let cases = [
        (
            "another event",
            "workflow_dispatch",
            "refs/heads/renovate/stomme",
            "29139614",
        ),
        ("another branch", "push", "refs/heads/main", "29139614"),
        ("another actor", "push", "refs/heads/renovate/stomme", "1"),
    ];

    for (name, event, reference, actor) in cases {
        let output = run_guard(&[
            ("EVENT_NAME", event),
            ("REF", reference),
            ("ACTOR_ID", actor),
        ]);

        assert_eq!(output, "run=false\n", "{name}");
    }
}

#[test]
fn ci_shards_the_mutation_changed_job_across_four_runners() {
    let job = workflow_job(CI, "mutation-shard");

    assert!(job.contains("\n    name: Mutation (changed) shard ${{ matrix.shard }}\n"));
    assert!(job.contains("\n    strategy:\n"));
    assert!(job.contains("\n      fail-fast: false\n"));
    assert!(job.contains("\n      matrix:\n        shard: [0, 1, 2, 3]\n"));

    let steps = job_steps(CI, "mutation-shard");
    let mutate = workflow_step(&steps, "Mutate the pull request's diff");
    let script = step_script(mutate);
    let mutate_at = script
        .find("cargo xtask mutation changed \"$BASE_SHA\" --shard \"$SHARD/4\"")
        .unwrap_or_else(|| panic!("the step runs one shard: {script}"));
    let memory_start = script.find("free -m").expect("the step logs memory first");
    let memory_end = script.rfind("free -m").expect("the step logs memory last");

    assert!(mutate.contains("\n          SHARD: ${{ matrix.shard }}\n"));
    assert!(memory_start < mutate_at, "{script}");
    assert!(mutate_at < memory_end, "{script}");
    assert_eq!(script.matches("free -m").count(), 2, "{script}");
    assert_eq!(script.matches("df -h").count(), 2, "{script}");

    let upload = steps
        .iter()
        .find(|step| step.contains("actions/upload-artifact"))
        .expect("the shard uploads its report");

    assert!(upload.contains("\n        if: always()\n"));
    assert!(upload.contains("name: mutation-shard-${{ matrix.shard }}"));
    assert!(upload.contains("mutation-report.json"));
    assert!(upload.contains("target/mutants/mutants.out/debug.log"));
}

#[test]
fn ci_aggregates_the_mutation_shards_under_the_gate_name() {
    let named: Vec<&str> = workflow_jobs(CI)
        .into_iter()
        .filter(|(_, job)| {
            job.lines()
                .any(|line| line.trim() == "name: Mutation (changed)")
        })
        .map(|(job, _)| job)
        .collect();

    assert_eq!(named, vec!["mutation"], "one job carries the gate name");

    let job = workflow_job(CI, "mutation");

    assert!(job.contains("\n    if: always() && github.event_name == 'pull_request'\n"));
    assert!(job.contains("\n    needs: [mutation-shard]\n"));

    let steps = job_steps(CI, "mutation");
    let download = steps
        .iter()
        .find(|step| step.contains("actions/download-artifact"))
        .expect("the aggregate downloads the shard reports");
    let failure = steps
        .iter()
        .find(|step| step.contains("needs.mutation-shard.result"))
        .expect("the aggregate fails when a shard failed");

    assert!(download.contains("pattern: mutation-shard-*"));
    assert!(failure.contains("\n        if: needs.mutation-shard.result != 'success'\n"));
    assert!(failure.contains("exit 1"));
    assert!(
        workflow_job(CI, "mutation-audit")
            .contains("cargo mutants --workspace --test-tool nextest")
    );
}

#[test]
fn ci_merges_the_shard_reports() {
    let steps = job_steps(CI, "mutation");
    let merge = workflow_step(&steps, "Merge the shard reports");
    let cases = [
        (
            "survivors from two shards",
            vec![
                report("survivors", vec![survivor("a.rs", 1)], &[]),
                report("survivors", vec![survivor("b.rs", 2)], &[]),
            ],
            "survivors",
            vec!["a.rs:1", "b.rs:2"],
        ),
        (
            "an error in one shard",
            vec![report("pass", vec![], &[]), report("error", vec![], &[])],
            "error",
            vec![],
        ),
        (
            "no mutants in any shard",
            vec![
                report("nothing", vec![], &[]),
                report("nothing", vec![], &[]),
            ],
            "nothing",
            vec![],
        ),
    ];

    for (name, reports, status, listed) in cases {
        let (merged, summary) = run_merge(merge, &reports);

        assert_eq!(merged["status"], status, "{name}");
        assert_eq!(
            merged["survivors"].as_array().map(Vec::len),
            Some(listed.len()),
            "{name}"
        );

        for line in listed {
            assert!(summary.contains(line), "{name}: {summary}");
        }
    }
}

fn run_merge(step: &str, reports: &[String]) -> (Value, String) {
    let directory = temporary_directory();
    let root = directory.path();
    let summary = root.join("summary.md");

    for (index, report) in reports.iter().enumerate() {
        let artifact = root.join(format!("shards/mutation-shard-{index}"));

        fs::create_dir_all(&artifact).expect("create the artifact folder");
        fs::write(artifact.join("mutation-report.json"), report).expect("write the report");
    }

    let output = isolated_command("bash", root)
        .arg("-e")
        .arg("-c")
        .arg(step_script(step))
        .env("GITHUB_STEP_SUMMARY", &summary)
        .output()
        .expect("run the merge step");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "{stdout}{stderr}");

    let merged = fs::read_to_string(root.join("mutation-report.json")).expect("read the report");
    let merged = serde_json::from_str(&merged).expect("the merged report is JSON");
    let summary = fs::read_to_string(&summary).expect("read the job summary");

    (merged, summary)
}

fn report(status: &str, survivors: Vec<Value>, unmutated: &[&str]) -> String {
    json!({
        "version": 1,
        "status": status,
        "survivors": survivors,
        "unmutated": unmutated,
    })
    .to_string()
}

fn survivor(file: &str, line: usize) -> Value {
    json!({
        "file": file,
        "line": line,
        "mutator": "BinaryOperator",
        "original": "<=",
        "replacement": ">",
        "covered": true,
    })
}

fn run_guard(environment: &[(&str, &str)]) -> String {
    let directory = temporary_directory();
    let output = directory.path().join("output");

    fs::write(&output, "").expect("create the guard output file");

    let step = workflow_step(&job_steps(SYNC_UPDATE, "sync"), SYNC_GUARD);
    let result = isolated_command("bash", directory.path())
        .arg("-e")
        .arg("-c")
        .arg(step_script(step))
        .env("GITHUB_OUTPUT", &output)
        .envs(environment.iter().copied())
        .output()
        .expect("run the guard step");

    assert!(result.status.success(), "{result:?}");

    fs::read_to_string(&output).expect("read the guard output")
}

fn write_file(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    let parent = path.parent().expect("the file has a parent folder");

    fs::create_dir_all(parent).expect("create the file's folder");
    fs::write(&path, contents).expect("write the file");
}

fn run_step(step: &str, directory: &Path) -> Output {
    let script = step_script(step);

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

fn step_script(step: &str) -> String {
    let script = step
        .split_once("        run: |\n")
        .unwrap_or_else(|| panic!("the step runs a script: {step}"))
        .1;

    script
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
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

fn workflow_step_index(steps: &[&str], name: &str) -> usize {
    let header = format!("      - name: {name}\n");

    steps
        .iter()
        .position(|step| step.starts_with(header.as_str()))
        .unwrap_or_else(|| panic!("the workflow has a step named {name}"))
}

fn step_index_containing(steps: &[&str], marker: &str) -> usize {
    steps
        .iter()
        .position(|step| step.contains(marker))
        .unwrap_or_else(|| panic!("the workflow has a step with {marker}"))
}

fn step_if(step: &str) -> Option<&str> {
    step.lines()
        .find_map(|line| line.strip_prefix("        if: "))
}

fn workflow_jobs(workflow: &str) -> Vec<(&str, &str)> {
    let body = workflow
        .split_once("\njobs:\n")
        .unwrap_or_else(|| panic!("the workflow declares jobs"))
        .1;
    let mut jobs = Vec::new();
    let mut current: Option<(usize, &str)> = None;
    let mut offset = 0;

    for line in body.split_inclusive('\n') {
        if let Some(name) = job_header(line) {
            if let Some((start, previous)) = current.take() {
                jobs.push((previous, &body[start..offset]));
            }

            current = Some((offset, name));
        }

        offset += line.len();
    }

    if let Some((start, name)) = current {
        jobs.push((name, &body[start..]));
    }

    jobs
}

fn job_header(line: &str) -> Option<&str> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let name = line.strip_prefix("  ")?.strip_suffix(':')?;

    if name.is_empty() || name.starts_with(' ') || name.starts_with('#') {
        return None;
    }

    Some(name)
}

fn workflow_job<'a>(workflow: &'a str, name: &str) -> &'a str {
    workflow_jobs(workflow)
        .into_iter()
        .find(|(job, _)| *job == name)
        .unwrap_or_else(|| panic!("the workflow has a job named {name}"))
        .1
}

fn job_steps<'a>(workflow: &'a str, job: &str) -> Vec<&'a str> {
    workflow_steps(workflow_job(workflow, job))
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = isolated_command("git", directory)
        .args(arguments)
        .env("GIT_AUTHOR_NAME", "stomme test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "stomme test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("run git");

    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout)
        .expect("git prints UTF-8")
        .trim_end()
        .to_string()
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("the path is UTF-8")
}

fn create_commit(
    directory: &Path,
    file: &str,
    contents: &str,
    message: &str,
    parent: Option<&str>,
) -> String {
    write_file(directory, file, contents);
    git(directory, &["add", "--", file]);

    let tree = git(directory, &["write-tree"]);
    let mut arguments = vec!["commit-tree", tree.as_str(), "-m", message];

    if let Some(parent) = parent {
        arguments.push("-p");
        arguments.push(parent);
    }

    let commit = git(directory, &arguments);
    git(directory, &["update-ref", "HEAD", &commit]);

    commit
}

fn bare_remote(root: &Path) -> PathBuf {
    let remote = root.join("remote.git");

    git(root, &["init", "--quiet", "--bare", path_text(&remote)]);
    git(
        root,
        &[
            "-C",
            path_text(&remote),
            "config",
            "uploadpack.allowAnySHA1InWant",
            "true",
        ],
    );

    remote
}

fn work_repository(root: &Path) -> PathBuf {
    let work = root.join("work");

    fs::create_dir(&work).expect("create the work folder");
    git(&work, &["init", "--quiet", "."]);

    work
}

fn bundle_repository(root: &Path) -> (PathBuf, String, String, String) {
    let remote = bare_remote(root);
    let work = work_repository(root);
    let base = create_commit(&work, "file.txt", "base\n", "base", None);

    git(
        &work,
        &[
            "push",
            "--quiet",
            path_text(&remote),
            "HEAD:refs/heads/renovate/stomme",
        ],
    );

    let tip = create_commit(
        &work,
        "file.txt",
        "base\ntip\n",
        "chore: sync stomme files",
        Some(&base),
    );
    let tip2 = create_commit(
        &work,
        "file.txt",
        "base\ntip\ntip2\n",
        "chore: sync stomme files",
        Some(&tip),
    );

    git(&work, &["update-ref", "refs/heads/tip", &tip]);
    git(&work, &["update-ref", "refs/heads/tip2", &tip2]);

    (remote, base, tip, tip2)
}

fn sibling_bundle_repository(root: &Path) -> (PathBuf, String, String) {
    let remote = bare_remote(root);
    let work = work_repository(root);
    let ancestor = create_commit(&work, "file.txt", "ancestor\n", "ancestor", None);
    let base = create_commit(
        &work,
        "file.txt",
        "ancestor\nbase\n",
        "base",
        Some(&ancestor),
    );

    git(
        &work,
        &[
            "push",
            "--quiet",
            path_text(&remote),
            "HEAD:refs/heads/renovate/stomme",
        ],
    );

    let tip = create_commit(
        &work,
        "file.txt",
        "ancestor\ntip\n",
        "chore: sync stomme files",
        Some(&ancestor),
    );

    git(&work, &["update-ref", "refs/heads/tip", &tip]);

    (remote, base, tip)
}

fn bundle_range(work: &Path, destination: &Path, range: &str) {
    git(work, &["bundle", "create", path_text(destination), range]);
}

fn fake_gh(root: &Path) -> PathBuf {
    let bin = root.join("bin");
    let gh = bin.join("gh");

    fs::create_dir_all(&bin).expect("create the fake bin folder");
    fs::write(
        &gh,
        "#!/bin/sh\ncase \"$2\" in\n  /users/*) printf '42\\n' ;;\n  *) cat \"$FAKE_ARTIFACT\" ;;\nesac\n",
    )
    .expect("write the fake gh");

    let mut permissions = fs::metadata(&gh).expect("read the fake gh").permissions();

    permissions.set_mode(0o755);
    fs::set_permissions(&gh, permissions).expect("make the fake gh executable");

    bin
}

fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut directory = Vec::new();

    for (name, contents) in entries {
        let name = name.as_bytes();
        let size = u32::try_from(contents.len()).expect("the entry fits a ZIP");
        let offset = u32::try_from(archive.len()).expect("the archive fits a ZIP");
        let name_length = u16::try_from(name.len()).expect("the name fits a ZIP");
        let crc = crc32(contents);

        archive.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&size.to_le_bytes());
        archive.extend_from_slice(&size.to_le_bytes());
        archive.extend_from_slice(&name_length.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(name);
        archive.extend_from_slice(contents);

        directory.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        directory.extend_from_slice(&20u16.to_le_bytes());
        directory.extend_from_slice(&20u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&crc.to_le_bytes());
        directory.extend_from_slice(&size.to_le_bytes());
        directory.extend_from_slice(&size.to_le_bytes());
        directory.extend_from_slice(&name_length.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u32.to_le_bytes());
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name);
    }

    let directory_offset = u32::try_from(archive.len()).expect("the archive fits a ZIP");
    let directory_size = u32::try_from(directory.len()).expect("the directory fits a ZIP");
    let count = u16::try_from(entries.len()).expect("the entries fit a ZIP");

    archive.extend_from_slice(&directory);
    archive.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&directory_size.to_le_bytes());
    archive.extend_from_slice(&directory_offset.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());

    archive
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;

    for byte in bytes {
        crc ^= u32::from(*byte);

        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();

            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }

    !crc
}

fn write_git_config(directory: &Path, remote: &Path) -> PathBuf {
    let config = directory.join("gitconfig");

    fs::write(
        &config,
        format!(
            "[url \"file://{}\"]\n\tinsteadOf = https://github.com/test/repo.git\n",
            remote.display()
        ),
    )
    .expect("write the git config");

    config
}

fn run_script(
    step: &str,
    directory: &Path,
    bin: &Path,
    config: &Path,
    environment: &[(&str, &str)],
) -> Output {
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").expect("PATH is set")
    );

    isolated_command("bash", directory)
        .arg("-e")
        .arg("-c")
        .arg(step_script(step))
        .env("PATH", path)
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(environment.iter().copied())
        .output()
        .expect("run the workflow step")
}

fn run_bundle_check(
    step: &str,
    bin: &Path,
    config: &Path,
    bundle: &Path,
    expected_sha: &str,
    expected_commit: &str,
) -> (Output, String) {
    let directory = temporary_directory();
    let output = directory.path().join("output");
    let contents = fs::read(bundle).expect("read the bundle");
    let archive = zip_stored(&[("sync.bundle", contents.as_slice())]);

    fs::write(&output, "").expect("create the step output file");
    fs::write(directory.path().join("sync-artifact.zip"), archive).expect("write the artifact");

    let result = run_script(
        step,
        directory.path(),
        bin,
        config,
        &[
            ("WORK", path_text(directory.path())),
            ("GITHUB_TOKEN", "token"),
            ("REPOSITORY", "test/repo"),
            ("EXPECTED_SHA", expected_sha),
            ("EXPECTED_COMMIT", expected_commit),
            ("GITHUB_OUTPUT", path_text(&output)),
        ],
    );
    let written = fs::read_to_string(&output).expect("read the step output");

    (result, written)
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

const ANY_TAG: &str = "*";
const RELEASE_TAG: (&str, &str) = ("v-release.yml", "v**[0-9]+.[0-9]+.[0-9]+*");
const RELEASE_TRIGGER: &str =
    "on:\n  pull_request:\n  push:\n    tags:\n      - 'v**[0-9]+.[0-9]+.[0-9]+*'\n";

fn is_workflow(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "yml" || extension == "yaml")
}

fn release_tag_problems(directory: &Path) -> Vec<String> {
    let mut problems = Vec::new();

    if directory.join("release.yml").exists() {
        problems
            .push("release.yml still exists; the Release workflow is v-release.yml".to_string());
    }

    let (release_file, release_pattern) = RELEASE_TAG;
    let mut starts_on_the_release_tag = false;

    for (name, workflow) in workflow_files(directory) {
        let patterns = match tag_push_patterns(&workflow) {
            Ok(patterns) => patterns,
            Err(error) => {
                problems.push(format!("{name}: {error}"));
                continue;
            }
        };

        for pattern in patterns {
            if name == release_file && pattern == release_pattern {
                starts_on_the_release_tag = true;
                continue;
            }

            problems.push(format!(
                "{name} can start on a tag push matching '{pattern}'"
            ));
        }
    }

    if !starts_on_the_release_tag {
        problems.push(format!(
            "{release_file} has no '{release_pattern}' tag push"
        ));
    }

    problems
}

fn workflow_files(directory: &Path) -> Vec<(String, String)> {
    let mut paths: Vec<PathBuf> = fs::read_dir(directory)
        .expect("read the workflows folder")
        .map(|entry| entry.expect("read a workflow entry").path())
        .filter(|path| is_workflow(path))
        .collect();

    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .expect("a workflow file has a name")
                .to_string_lossy()
                .into_owned();
            let workflow = fs::read_to_string(&path).expect("read a workflow");

            (name, workflow)
        })
        .collect()
}

fn release_workflows(extra: &[(&str, &str)]) -> TempDir {
    let directory = temporary_directory();

    write_file(directory.path(), RELEASE_TAG.0, RELEASE_TRIGGER);

    for (name, workflow) in extra {
        write_file(directory.path(), name, workflow);
    }

    directory
}

#[derive(Debug, Error, PartialEq, Eq)]
enum TriggerError {
    #[error("the workflow declares no on: trigger")]
    NoTrigger,
    #[error("the guard cannot read the trigger line `{0}`")]
    Unreadable(String),
}

#[derive(Clone, Copy)]
struct TriggerLine<'a> {
    indent: usize,
    text: &'a str,
}

struct PushFilter<'a> {
    key: &'a str,
    values: Vec<String>,
    line: &'a str,
}

fn tag_push_patterns(workflow: &str) -> Result<Vec<String>, TriggerError> {
    let lines = trigger_lines(workflow);
    let trigger = lines
        .iter()
        .position(|line| line.indent == 0 && line.text.starts_with("on:"));

    let Some(index) = trigger else {
        return Err(TriggerError::NoTrigger);
    };

    let line = lines[index];
    let value = line
        .text
        .strip_prefix("on:")
        .expect("the line starts with on:");
    let value = strip_comment(value).trim();

    if value.is_empty() {
        return trigger_block_patterns(block_after(&lines, index));
    }

    event_patterns(value).ok_or_else(|| TriggerError::Unreadable(line.text.to_string()))
}

fn trigger_block_patterns(lines: &[TriggerLine<'_>]) -> Result<Vec<String>, TriggerError> {
    let mut patterns = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let children = block_after(lines, index);

        let Some((key, _)) = line.text.split_once(':') else {
            return Err(TriggerError::Unreadable(line.text.to_string()));
        };

        if strip_quotes(key.trim()) == "push" {
            patterns.extend(push_patterns(line.text, children)?);
        }

        index += 1 + children.len();
    }

    Ok(patterns)
}

fn block_after<'a>(lines: &'a [TriggerLine<'a>], index: usize) -> &'a [TriggerLine<'a>] {
    let parent = lines[index].indent;
    let rest = &lines[index + 1..];
    let length = rest
        .iter()
        .position(|line| line.indent <= parent)
        .unwrap_or(rest.len());

    &rest[..length]
}

fn push_patterns(line: &str, children: &[TriggerLine<'_>]) -> Result<Vec<String>, TriggerError> {
    let (_, value) = line
        .split_once(':')
        .expect("a push trigger line has a key and a value");
    let value = strip_comment(value).trim();

    if value.is_empty() {
        return filter_patterns(&block_filters(children)?);
    }

    let filters = flow_filters(line).ok_or_else(|| TriggerError::Unreadable(line.to_string()))?;

    filter_patterns(&filters)
}

fn block_filters<'a>(lines: &'a [TriggerLine<'a>]) -> Result<Vec<PushFilter<'a>>, TriggerError> {
    let mut filters: Vec<PushFilter<'a>> = Vec::new();
    let mut key_indent = None;

    for line in lines {
        if let Some(item) = list_item(line.text) {
            let Some(filter) = filters.last_mut() else {
                return Err(TriggerError::Unreadable(line.text.to_string()));
            };

            filter
                .values
                .push(strip_quotes(strip_comment(item)).to_string());
            continue;
        }

        let indent = *key_indent.get_or_insert(line.indent);

        if line.indent != indent {
            return Err(TriggerError::Unreadable(line.text.to_string()));
        }

        let Some((key, value)) = line.text.split_once(':') else {
            return Err(TriggerError::Unreadable(line.text.to_string()));
        };

        let Some(values) = filter_values(value) else {
            return Err(TriggerError::Unreadable(line.text.to_string()));
        };

        filters.push(PushFilter {
            key: strip_quotes(key.trim()),
            values,
            line: line.text,
        });
    }

    Ok(filters)
}

fn flow_filters(line: &str) -> Option<Vec<PushFilter<'_>>> {
    let (_, value) = line.split_once(':')?;
    let inner = strip_comment(value)
        .trim()
        .strip_prefix('{')?
        .strip_suffix('}')?;
    let mut filters = Vec::new();

    for entry in split_flow(inner) {
        let (key, value) = entry.split_once(':')?;

        filters.push(PushFilter {
            key: strip_quotes(key.trim()),
            values: filter_values(value)?,
            line,
        });
    }

    Some(filters)
}

fn filter_values(value: &str) -> Option<Vec<String>> {
    let value = strip_comment(value).trim();

    if value.is_empty() {
        return Some(Vec::new());
    }

    if value.starts_with('{') {
        return None;
    }

    if value.starts_with('[') {
        return inline_list(value);
    }

    Some(vec![strip_quotes(value).to_string()])
}

fn filter_patterns(filters: &[PushFilter<'_>]) -> Result<Vec<String>, TriggerError> {
    let mut tags: Option<&[String]> = None;
    let mut ignores_tags = false;
    let mut restricts_branches = false;

    for filter in filters {
        match filter.key {
            "tags" => tags = Some(&filter.values),
            "tags-ignore" => ignores_tags = true,
            "branches" | "branches-ignore" => restricts_branches = true,
            "paths" | "paths-ignore" => {}
            _ => return Err(TriggerError::Unreadable(filter.line.to_string())),
        }
    }

    if let Some(tags) = tags {
        return Ok(tags.to_vec());
    }

    if ignores_tags {
        return Ok(vec![ANY_TAG.to_string()]);
    }

    if restricts_branches {
        return Ok(Vec::new());
    }

    Ok(vec![ANY_TAG.to_string()])
}

fn event_patterns(value: &str) -> Option<Vec<String>> {
    let value = strip_quotes(value);

    if value == "push" {
        return Some(vec![ANY_TAG.to_string()]);
    }

    if let Some(events) = inline_list(value) {
        let starts_on_push = events.iter().any(|event| event == "push");

        return Some(if starts_on_push {
            vec![ANY_TAG.to_string()]
        } else {
            Vec::new()
        });
    }

    let is_event_name = value
        .chars()
        .all(|character| character.is_ascii_lowercase() || character == '_');

    if is_event_name {
        return Some(Vec::new());
    }

    None
}

fn split_flow(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;

    for (index, character) in text.char_indices() {
        match character {
            '[' | '{' => depth += 1,
            ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }

    parts.push(&text[start..]);

    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn trigger_lines(workflow: &str) -> Vec<TriggerLine<'_>> {
    workflow
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                return None;
            }

            let text = strip_comment(trimmed).trim();

            if text.is_empty() {
                return None;
            }

            Some(TriggerLine {
                indent: indentation(line),
                text,
            })
        })
        .collect()
}

fn list_item(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('-')?;

    if rest.starts_with(char::is_whitespace) {
        return Some(rest.trim());
    }

    None
}

fn inline_list(text: &str) -> Option<Vec<String>> {
    let inner = text.strip_prefix('[')?.strip_suffix(']')?;

    Some(
        inner
            .split(',')
            .map(|item| strip_quotes(strip_comment(item.trim())))
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn line_index(workflow: &str, needle: &str) -> usize {
    workflow
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle} is in the workflow"))
}
