use std::cell::RefCell;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};

use super::*;
use crate::test_support::{strings, temporary_directory};

const CONFIG: &str = "schema = 1\nversion = \"0.0.0\"\nlanguage = \"rust\"\n";

#[derive(Clone, Default)]
struct Response {
    fails: bool,
    stderr: String,
}

#[derive(Clone, Default)]
struct Fake {
    sync: Response,
    check: Response,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Call {
    folder: PathBuf,
    arguments: Vec<String>,
}

thread_local! {
    static FAKE: RefCell<Fake> = RefCell::new(Fake::default());
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

fn install(sync: Response, check: Response) {
    FAKE.with(|fake| *fake.borrow_mut() = Fake { sync, check });
    CALLS.with(|calls| calls.borrow_mut().clear());
}

fn calls() -> Vec<Call> {
    CALLS.with(|calls| calls.borrow().clone())
}

fn output(response: &Response) -> Output {
    let code = if response.fails { 1 } else { 0 };

    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: Vec::new(),
        stderr: response.stderr.clone().into_bytes(),
    }
}

fn fake_runner(_binary: &Path, folder: &Path, arguments: &[&str]) -> io::Result<Output> {
    let arguments: Vec<String> = arguments
        .iter()
        .map(|argument| argument.to_string())
        .collect();

    CALLS.with(|calls| {
        calls.borrow_mut().push(Call {
            folder: folder.to_path_buf(),
            arguments: arguments.clone(),
        });
    });

    let response = FAKE.with(|fake| {
        let fake = fake.borrow();

        match arguments.first().map(String::as_str) {
            Some("sync") => fake.sync.clone(),
            Some("check") => fake.check.clone(),
            _ => Response::default(),
        }
    });

    Ok(output(&response))
}

fn runner() -> Runner {
    Runner { run: fake_runner }
}

fn project(config: &str) -> tempfile::TempDir {
    let folder = temporary_directory();

    std::fs::write(folder.path().join("stomme.toml"), config).expect("write stomme.toml");

    folder
}

fn config_text(folder: &tempfile::TempDir) -> String {
    std::fs::read_to_string(folder.path().join("stomme.toml")).expect("read stomme.toml")
}

#[test]
fn gate_fails_an_adopted_project_whose_check_fails() {
    let folder = project(CONFIG);
    install(
        Response::default(),
        Response {
            fails: true,
            stderr: "stomme: x".to_string(),
        },
    );

    let outcome =
        gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(
        outcome,
        Outcome::Failed {
            step: "check",
            output: "stomme: x".to_string(),
        }
    );
    assert_eq!(exit_code(&outcome), ExitCode::FAILURE);
}

#[test]
fn gate_passes_an_adopted_project_after_sync_and_check() {
    let folder = project(CONFIG);
    install(Response::default(), Response::default());

    let outcome =
        gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(outcome, Outcome::Passed);
    assert_eq!(exit_code(&outcome), ExitCode::SUCCESS);

    let arguments: Vec<Vec<String>> = calls().into_iter().map(|call| call.arguments).collect();
    assert_eq!(
        arguments,
        vec![
            vec!["sync".to_string()],
            vec![
                "check".to_string(),
                "--base".to_string(),
                "HEAD".to_string()
            ],
        ]
    );
}

#[test]
fn gate_rewrites_the_pinned_version_before_sync() {
    let folder = project(
        "schema = 1\nversion = \"0.0.0\"\nlanguage = \"rust\"\n\n[roles]\nversion = \"9.9.9\"\n",
    );
    install(Response::default(), Response::default());

    gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    let config =
        std::fs::read_to_string(folder.path().join("stomme.toml")).expect("read stomme.toml");

    assert!(config.contains("version = \"1.2.3\""), "{config}");
    assert!(config.contains("[roles]\nversion = \"9.9.9\""), "{config}");
}

#[test]
fn gate_stops_after_a_failed_sync() {
    let folder = project(CONFIG);
    install(
        Response {
            fails: true,
            stderr: "stomme: y".to_string(),
        },
        Response::default(),
    );

    let outcome =
        gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(
        outcome,
        Outcome::Failed {
            step: "sync",
            output: "stomme: y".to_string(),
        }
    );

    let arguments: Vec<Vec<String>> = calls().into_iter().map(|call| call.arguments).collect();
    assert_eq!(arguments, [["sync"].map(str::to_string)]);
}

#[test]
fn gate_reports_a_project_without_stomme_toml_without_failing() {
    let folder = temporary_directory();
    install(
        Response::default(),
        Response {
            fails: true,
            stderr: "stomme: z".to_string(),
        },
    );

    let outcome =
        gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(
        outcome,
        Outcome::NotAdopted {
            output: "stomme: z".to_string(),
        }
    );
    assert_eq!(exit_code(&outcome), ExitCode::SUCCESS);
    assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 0);
}

#[test]
fn gate_refuses_a_stomme_toml_without_a_version_line() {
    let folder = project("schema = 1\nlanguage = \"rust\"\n");
    install(Response::default(), Response::default());

    let error = gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path())
        .expect_err("refuse the config");

    let message = format!("{error:#}");
    assert!(
        message.contains(&folder.path().display().to_string()),
        "{message}"
    );
    assert!(
        message.contains("has no top-level version line"),
        "{message}"
    );
    assert_eq!(config_text(&folder), "schema = 1\nlanguage = \"rust\"\n");
    assert!(calls().is_empty());
}

#[test]
fn gate_rewrites_a_commented_version_pin_and_keeps_the_comment() {
    let config = "schema = 1\nversion = \"0.2.0\" # candidate pin\nlanguage = \"rust\"\n";
    let folder = project(config);
    install(Response::default(), Response::default());

    let outcome =
        gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(outcome, Outcome::Passed);
    assert_eq!(
        config_text(&folder),
        "schema = 1\nversion = \"1.2.3\" # candidate pin\nlanguage = \"rust\"\n"
    );
}

#[test]
fn gate_rewrites_a_literal_string_version_pin() {
    let config = "schema = 1\nversion = '0.2.0'\nlanguage = \"rust\"\n";
    let folder = project(config);
    install(Response::default(), Response::default());

    let outcome =
        gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(outcome, Outcome::Passed);
    assert_eq!(
        config_text(&folder),
        "schema = 1\nversion = \"1.2.3\"\nlanguage = \"rust\"\n"
    );
}

#[test]
fn gate_refuses_a_stomme_toml_that_does_not_parse() {
    let config = "schema = 1\nversion = \"0.0.0\"\nlanguage = [rust\n";
    let folder = project(config);
    install(Response::default(), Response::default());

    let error = gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path())
        .expect_err("refuse the config");

    let message = format!("{error:#}");
    assert!(
        message.contains(&folder.path().display().to_string()),
        "{message}"
    );
    assert!(message.contains("does not parse as TOML"), "{message}");
    assert_eq!(config_text(&folder), config);
    assert!(calls().is_empty());
}

#[test]
fn gate_refuses_a_non_string_version() {
    let config = "schema = 1\nversion = 1\nlanguage = \"rust\"\n";
    let folder = project(config);
    install(Response::default(), Response::default());

    let error = gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path())
        .expect_err("refuse the config");

    let message = format!("{error:#}");
    assert!(
        message.contains(&folder.path().display().to_string()),
        "{message}"
    );
    assert!(message.contains("not a string"), "{message}");
    assert_eq!(config_text(&folder), config);
    assert!(calls().is_empty());
}

#[test]
fn parse_arguments_reads_the_flags_and_rejects_wrong_ones() {
    let arguments = strings(&["--stomme", "/bin/stomme", "--version", "1.2.3", "project"]);

    assert_eq!(
        parse_arguments(&arguments),
        Some(Arguments {
            binary: PathBuf::from("/bin/stomme"),
            version: "1.2.3".to_string(),
            folder: PathBuf::from("project"),
        })
    );

    for arguments in [
        strings(&["--other", "/bin/stomme", "--version", "1.2.3", "project"]),
        strings(&["--stomme", "/bin/stomme", "--other", "1.2.3", "project"]),
    ] {
        assert_eq!(parse_arguments(&arguments), None, "{arguments:?}");
    }
}

#[test]
fn gate_keeps_the_indent_of_an_indented_version_line() {
    let folder = project("schema = 1\n  version = \"0.0.0\"\nlanguage = \"rust\"\n");
    install(Response::default(), Response::default());

    gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    let config =
        std::fs::read_to_string(folder.path().join("stomme.toml")).expect("read stomme.toml");

    assert!(config.contains("  version = \"1.2.3\""), "{config}");
}

#[test]
fn report_renders_the_heading_and_each_outcome() {
    let folder = Path::new("/projects/example");
    let cases = [
        (Outcome::Passed, "### example\nPassed\n".to_string()),
        (
            Outcome::Failed {
                step: "check",
                output: "stomme: x\n".to_string(),
            },
            "### example\nFailed at check\n```text\nstomme: x\n```\n".to_string(),
        ),
        (
            Outcome::NotAdopted {
                output: "stomme: y".to_string(),
            },
            "### example\nNot adopted; reported only\n```text\nstomme: y\n```\n".to_string(),
        ),
    ];

    for (outcome, expected) in cases {
        assert_eq!(report(folder, &outcome), expected, "{outcome:?}");
    }
}
