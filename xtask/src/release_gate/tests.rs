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
    static SYNC_CONFIG: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn install(sync: Response, check: Response) {
    FAKE.with(|fake| *fake.borrow_mut() = Fake { sync, check });
    CALLS.with(|calls| calls.borrow_mut().clear());
    SYNC_CONFIG.with(|config| *config.borrow_mut() = None);
}

fn calls() -> Vec<Call> {
    CALLS.with(|calls| calls.borrow().clone())
}

fn sync_config() -> Option<String> {
    SYNC_CONFIG.with(|config| config.borrow().clone())
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

    if arguments.first().map(String::as_str) == Some("sync") {
        let config = std::fs::read_to_string(folder.join("stomme.toml"))
            .expect("the test wrote stomme.toml before running the gate");
        SYNC_CONFIG.with(|recorded| *recorded.borrow_mut() = Some(config));
    }

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
    Runner {
        run: fake_runner,
        inspect: inspect_entry,
    }
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

enum Entry {
    Absent,
    RegularFile,
    BrokenLink,
    LinkLoop,
    FolderIsAFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Inspect,
    Read,
}

enum Expected {
    NotAdopted,
    Passed,
    Refused(Failure),
}

struct Fixture {
    folder: PathBuf,
    config: PathBuf,
    missing_target: PathBuf,
}

fn fixture(entry: &Entry, scratch: &Path) -> Fixture {
    use std::os::unix::fs::symlink;

    let missing_target = scratch.join("missing.toml");
    let folder = match entry {
        Entry::FolderIsAFile => {
            let file = scratch.join("file");
            std::fs::write(&file, "contents").expect("write the file");
            file
        }
        _ => scratch.to_path_buf(),
    };
    let config = folder.join("stomme.toml");

    match entry {
        Entry::Absent | Entry::FolderIsAFile => {}
        Entry::RegularFile => std::fs::write(&config, CONFIG).expect("write stomme.toml"),
        Entry::BrokenLink => symlink(&missing_target, &config).expect("create the link"),
        Entry::LinkLoop => symlink(&config, &config).expect("create the link loop"),
    }

    Fixture {
        folder,
        config,
        missing_target,
    }
}

fn assert_entry_unchanged(entry: &Entry, fixture: &Fixture, name: &str) {
    match entry {
        Entry::BrokenLink => {
            let target = std::fs::read_link(&fixture.config).expect("the link stays");
            assert_eq!(target, fixture.missing_target, "{name}");
            assert!(
                std::fs::symlink_metadata(&fixture.missing_target).is_err(),
                "{name}: the dangling target stays absent"
            );
        }
        Entry::LinkLoop => {
            let target = std::fs::read_link(&fixture.config).expect("the link stays");
            assert_eq!(target, fixture.config, "{name}");
        }
        Entry::FolderIsAFile => {
            let text = std::fs::read_to_string(&fixture.folder).expect("the file stays");
            assert_eq!(text, "contents", "{name}");
        }
        Entry::Absent | Entry::RegularFile => {}
    }
}

fn assert_refusal(error: &anyhow::Error, failure: Failure, config: &Path, name: &str) {
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains(&config.display().to_string()),
        "{name}: {rendered}"
    );

    match (error.downcast_ref::<ConfigError>(), failure) {
        (Some(ConfigError::Inspect { path, source }), Failure::Inspect)
        | (Some(ConfigError::Read { path, source }), Failure::Read) => {
            assert_eq!(path, config, "{name}");
            assert!(rendered.contains(&source.to_string()), "{name}: {rendered}");
        }
        (other, _) => panic!("{name}: expected {failure:?}, got {other:?}"),
    }
}

#[test]
fn gate_decides_adoption_from_the_stomme_toml_entry() {
    let table = [
        ("absent", Entry::Absent, Expected::NotAdopted),
        ("regular file", Entry::RegularFile, Expected::Passed),
        (
            "broken link",
            Entry::BrokenLink,
            Expected::Refused(Failure::Read),
        ),
        (
            "link loop",
            Entry::LinkLoop,
            Expected::Refused(Failure::Read),
        ),
        (
            "folder is a file",
            Entry::FolderIsAFile,
            Expected::Refused(Failure::Inspect),
        ),
    ];

    for (name, entry, expected) in table {
        let scratch = temporary_directory();
        let fixture = fixture(&entry, scratch.path());

        install(Response::default(), Response::default());

        let result = gate(&runner(), Path::new("/stomme"), "1.2.3", &fixture.folder);
        let sync_calls = calls()
            .iter()
            .filter(|call| call.arguments.first().map(String::as_str) == Some("sync"))
            .count();

        match expected {
            Expected::NotAdopted => {
                let outcome = result.expect("run the gate");
                assert!(matches!(outcome, Outcome::NotAdopted { .. }), "{name}");
                assert_eq!(exit_code(&outcome), ExitCode::SUCCESS, "{name}");
                assert_eq!(sync_calls, 0, "{name}");
            }
            Expected::Passed => {
                let outcome = result.expect("run the gate");
                assert_eq!(outcome, Outcome::Passed, "{name}");
                assert_eq!(exit_code(&outcome), ExitCode::SUCCESS, "{name}");
                assert_eq!(sync_calls, 1, "{name}");
            }
            Expected::Refused(failure) => {
                let error = result.expect_err(name);
                assert_refusal(&error, failure, &fixture.config, name);
                assert!(calls().is_empty(), "{name}");
                assert_entry_unchanged(&entry, &fixture, name);
            }
        }
    }
}

#[test]
fn gate_reports_a_different_cause_for_a_link_loop_than_for_a_broken_link() {
    let causes: Vec<io::ErrorKind> = [Entry::BrokenLink, Entry::LinkLoop]
        .iter()
        .map(|entry| {
            let scratch = temporary_directory();
            let fixture = fixture(entry, scratch.path());
            install(Response::default(), Response::default());

            let error = gate(&runner(), Path::new("/stomme"), "1.2.3", &fixture.folder)
                .expect_err("refuse the link");

            match error.downcast_ref::<ConfigError>() {
                Some(ConfigError::Read { source, .. }) => source.kind(),
                other => panic!("expected a read failure, got {other:?}"),
            }
        })
        .collect();

    assert_eq!(causes[0], io::ErrorKind::NotFound);
    assert_ne!(causes[1], io::ErrorKind::NotFound);
}

fn denied_inspect(_path: &Path) -> io::Result<std::fs::Metadata> {
    Err(io::Error::from(io::ErrorKind::PermissionDenied))
}

#[test]
fn gate_refuses_a_stomme_toml_that_cannot_be_inspected() {
    let folder = project(CONFIG);
    let config = folder.path().join("stomme.toml");
    install(Response::default(), Response::default());
    let runner = Runner {
        inspect: denied_inspect,
        ..runner()
    };

    let error =
        gate(&runner, Path::new("/stomme"), "1.2.3", folder.path()).expect_err("refuse the entry");

    let Some(ConfigError::Inspect { path, source }) = error.downcast_ref::<ConfigError>() else {
        panic!("expected an inspect failure, got {error:?}");
    };
    assert_eq!(path, &config);
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
    assert!(format!("{error:#}").contains(&config.display().to_string()));
    assert!(calls().is_empty());
    assert_eq!(config_text(&folder), CONFIG);
}

#[test]
fn gate_refuses_a_stomme_toml_without_a_version_line() {
    let folder = project("schema = 1\nlanguage = \"rust\"\n");
    install(Response::default(), Response::default());

    let error = gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path())
        .expect_err("refuse the config");

    let message = format!("{error:#}");
    assert_eq!(
        message,
        format!(
            "stomme.toml in {} has no top-level version line",
            folder.path().display()
        )
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

    let rewritten = "schema = 1\nversion = \"1.2.3\" # candidate pin\nlanguage = \"rust\"\n";
    assert_eq!(outcome, Outcome::Passed);
    assert_eq!(sync_config().as_deref(), Some(rewritten));
    assert_eq!(config_text(&folder), rewritten);
}

#[test]
fn gate_keeps_crlf_line_endings_when_rewriting_the_pin() {
    let config = "schema = 1\r\nversion = \"0.2.0\"\r\nlanguage = \"rust\"\r\n";
    let folder = project(config);
    install(Response::default(), Response::default());

    gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(
        config_text(&folder),
        "schema = 1\r\nversion = \"1.2.3\"\r\nlanguage = \"rust\"\r\n"
    );
}

#[test]
fn gate_keeps_a_missing_final_newline_when_rewriting_the_pin() {
    let config = "schema = 1\nversion = \"0.2.0\"\nlanguage = \"rust\"";
    let folder = project(config);
    install(Response::default(), Response::default());

    gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(
        config_text(&folder),
        "schema = 1\nversion = \"1.2.3\"\nlanguage = \"rust\""
    );
}

#[test]
fn gate_keeps_interleaved_dotted_keys_in_place_when_rewriting_the_pin() {
    let config = "schema = 1\nroles.cli = \"a\"\nversion = \"0.2.0\"\nlanguage = \"rust\"\nroles.other = \"b\"\n";
    let folder = project(config);
    install(Response::default(), Response::default());

    gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path()).expect("run the gate");

    assert_eq!(
        config_text(&folder),
        "schema = 1\nroles.cli = \"a\"\nversion = \"1.2.3\"\nlanguage = \"rust\"\nroles.other = \"b\"\n"
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
    let prefix = format!(
        "stomme.toml in {} does not parse as TOML",
        folder.path().display()
    );
    assert!(message.starts_with(&prefix), "{message}");
    assert!(message.contains("TOML parse error"), "{message}");

    let causes: Vec<String> = error.chain().map(|cause| cause.to_string()).collect();
    assert!(causes.len() > 1, "{causes:?}");
    assert_eq!(causes[0], prefix);
    assert!(causes[1].contains("TOML parse error"), "{causes:?}");

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
    assert_eq!(
        message,
        format!(
            "stomme.toml in {} has a version that is not a string",
            folder.path().display()
        )
    );
    assert_eq!(config_text(&folder), config);
    assert!(calls().is_empty());
}

#[test]
fn gate_refuses_a_table_valued_version() {
    let config = "schema = 1\nlanguage = \"rust\"\n\n[version]\npin = \"0.2.0\"\n";
    let folder = project(config);
    install(Response::default(), Response::default());

    let error = gate(&runner(), Path::new("/stomme"), "1.2.3", folder.path())
        .expect_err("refuse the config");

    let message = format!("{error:#}");
    assert_eq!(
        message,
        format!(
            "stomme.toml in {} has a version that is not a string",
            folder.path().display()
        )
    );
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
