use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn temporary_directory() -> TempDir {
    tempfile::Builder::new()
        .prefix("stomme-ocr-smoke-caller-")
        .tempdir()
        .expect("create a temporary directory")
}

const REPOSITORY_VARIABLES: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CEILING_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_GRAFT_FILE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_NAMESPACE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_OBJECT_DIRECTORY",
    "GIT_PREFIX",
    "GIT_REPLACE_REF_BASE",
    "GIT_SHALLOW_FILE",
    "GIT_WORK_TREE",
];

const OCR_PASSED_LINES: [&str; 5] = [
    "ocr-smoke: repository: admission passed",
    "ocr-smoke: repository: precedence passed",
    "ocr-smoke: typescript consumer: admission passed",
    "ocr-smoke: typescript consumer: precedence passed",
    "ocr-smoke: scope: admission passed",
];

fn git(folder: &Path, arguments: &[&str]) {
    let mut command = Command::new("git");

    command
        .current_dir(folder)
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    for name in REPOSITORY_VARIABLES {
        command.env_remove(name);
    }

    let status = command.status().expect("run git");
    assert!(status.success(), "git {} failed", arguments.join(" "));
}

fn assert_ocr_passed(stderr: &str) {
    for line in OCR_PASSED_LINES {
        assert!(stderr.contains(line), "{stderr}");
    }

    assert!(
        !stderr.contains("ocr-smoke: scope: precedence passed"),
        "{stderr}"
    );
}

#[test]
fn ocr_smoke_leaves_the_callers_index_unchanged() {
    let caller = temporary_directory();
    let index = caller.path().join(".git/index");

    git(caller.path(), &["init", "-q"]);
    fs::write(caller.path().join("keep.txt"), "keep").expect("write the caller file");
    git(caller.path(), &["add", "keep.txt"]);
    let before = fs::read(&index).expect("read the caller index");

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("ocr-smoke")
        .current_dir(caller.path())
        .env("GIT_INDEX_FILE", &index)
        .output()
        .expect("run ocr-smoke");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "{stderr}");
    assert_ocr_passed(&stderr);

    let after = fs::read(&index).expect("read the caller index");
    assert_eq!(after, before);
}

#[test]
fn ocr_smoke_ignores_an_inherited_git_config_override() {
    let folder = temporary_directory();
    let excludes = folder.path().join("excludes");
    fs::write(
        &excludes,
        "crates/stomme-cli/tests/fixtures/sample/src/lib.rs\n",
    )
    .expect("write the excludes file");

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("ocr-smoke")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.excludesFile")
        .env("GIT_CONFIG_VALUE_0", &excludes)
        .output()
        .expect("run ocr-smoke");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "{stderr}");
    assert_ocr_passed(&stderr);
}

#[test]
fn mutation_without_a_mode_fails_through_the_entry_point() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("mutation")
        .output()
        .expect("run mutation");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!output.status.success(), "{stdout}");
    assert!(stdout.contains("Name a mode"), "{stdout}");
}
