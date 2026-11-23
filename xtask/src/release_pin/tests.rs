use std::cell::RefCell;
use std::ffi::OsStr;
use std::io;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};

use super::*;
use crate::test_support::{strings, temporary_directory};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER_COMMIT: &str = "89abcdef0123456789abcdef0123456789abcdef";
const VERSION: &str = "1.2.3";

fn pin_line(commit: &str, version: &str) -> String {
    format!("{PIN_PREFIX}{commit} # v{version}")
}

fn workflow(pin: &str) -> String {
    format!("name: Sync stomme\n\njobs:\n  sync:\n    {pin}\n")
}

#[derive(Clone, Default)]
struct Fake {
    fails: bool,
    stderr: String,
}

thread_local! {
    static FAKE: RefCell<Fake> = RefCell::new(Fake::default());
    static READS: RefCell<usize> = const { RefCell::new(0) };
}

fn install(fake: Fake) {
    FAKE.with(|installed| *installed.borrow_mut() = fake);
    READS.with(|reads| *reads.borrow_mut() = 0);
}

fn read_count() -> usize {
    READS.with(|reads| *reads.borrow())
}

fn output(code: i32, stderr: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

fn fake_run(_binary: &Path, _folder: &Path) -> io::Result<Output> {
    let fake = FAKE.with(|fake| fake.borrow().clone());

    if fake.fails {
        return Ok(output(1, &fake.stderr));
    }

    Ok(output(0, ""))
}

fn counting_read(path: &Path) -> io::Result<String> {
    READS.with(|reads| *reads.borrow_mut() += 1);

    std::fs::read_to_string(path)
}

fn runner() -> Runner {
    Runner {
        run: fake_run,
        read: counting_read,
    }
}

#[test]
fn release_pin_passes_when_the_workflow_pins_the_commit() {
    let text = workflow(&pin_line(COMMIT, VERSION));

    assert_eq!(compare_pin(&text, VERSION, COMMIT), Outcome::Passed);
}

#[test]
fn release_pin_fails_when_the_commit_differs() {
    let found = pin_line(OTHER_COMMIT, VERSION);
    let text = workflow(&found);

    let outcome = compare_pin(&text, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains(&pin_line(COMMIT, VERSION)), "{output}");
    assert!(output.contains(&found), "{output}");
}

#[test]
fn release_pin_fails_for_the_tag_form() {
    let found = format!("{PIN_PREFIX}v{VERSION}");
    let text = workflow(&found);

    let outcome = compare_pin(&text, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains(&found), "{output}");
}

#[test]
fn release_pin_fails_when_the_version_comment_differs() {
    let found = format!("{PIN_PREFIX}{COMMIT} # v0.0.1");
    let text = workflow(&found);

    let outcome = compare_pin(&text, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains(&found), "{output}");
}

#[test]
fn release_pin_ignores_a_line_that_only_mentions_the_workflow() {
    let text = "jobs:\n  sync:\n    run: cat .github/workflows/sync-update.yml\n";

    let outcome = compare_pin(text, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(
        output.contains("Found no `uses:` line for sync-update.yml."),
        "{output}"
    );
    assert!(!output.contains("run: cat"), "{output}");
}

#[test]
fn release_pin_reports_a_failed_init() {
    let folder = temporary_directory();
    install(Fake {
        fails: true,
        stderr: "stomme: boom".to_string(),
    });

    let outcome = check_pin(
        &runner(),
        Path::new("/stomme"),
        VERSION,
        COMMIT,
        folder.path(),
    )
    .expect("run the check");

    assert_eq!(read_count(), 0);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected an init failure");
    };

    assert_eq!(step, "init");
    assert_eq!(output, "stomme: boom");
}

#[test]
fn release_pin_reports_a_missing_workflow() {
    let folder = temporary_directory();
    install(Fake::default());

    let outcome = check_pin(
        &runner(),
        Path::new("/stomme"),
        VERSION,
        COMMIT,
        folder.path(),
    )
    .expect("run the check");

    assert_eq!(read_count(), 1);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");

    let workflow = folder.path().join(WORKFLOW);
    assert!(output.contains(&workflow.display().to_string()), "{output}");
}

#[test]
fn release_pin_accepts_one_matching_pin_among_several() {
    let other = pin_line(OTHER_COMMIT, VERSION);
    let expected = pin_line(COMMIT, VERSION);
    let text = format!("jobs:\n  sync:\n    {other}\n  copy:\n    {expected}\n");

    assert_eq!(compare_pin(&text, VERSION, COMMIT), Outcome::Passed);
}

#[test]
fn release_pin_shows_every_sync_update_pin_on_failure() {
    let first = pin_line(OTHER_COMMIT, VERSION);
    let second = format!("{PIN_PREFIX}v{VERSION}");
    let text = format!("jobs:\n  sync:\n    {first}\n  copy:\n    {second}\n");

    let outcome = compare_pin(&text, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains(&first), "{output}");
    assert!(output.contains(&second), "{output}");
}

#[test]
fn release_pin_removes_the_commit_override() {
    let command = init_command(Path::new("/stomme"), Path::new("/project"));

    let entry = command
        .get_envs()
        .find(|(key, _)| key.to_str() == Some(COMMIT_OVERRIDE));

    assert_eq!(entry, Some((OsStr::new(COMMIT_OVERRIDE), None)));
}

#[test]
fn release_pin_runs_init_with_the_gate_name() {
    let command = init_command(Path::new("/stomme"), Path::new("/project"));

    let arguments: Vec<&str> = command
        .get_args()
        .map(|argument| argument.to_str().expect("the arguments are UTF-8"))
        .collect();

    assert_eq!(arguments, ["init", "--name", "gate", "--language", "rust"]);
    assert_eq!(command.get_current_dir(), Some(Path::new("/project")));
    assert_eq!(command.get_program(), "/stomme");
}

#[test]
fn parse_arguments_requires_a_commit() {
    let missing = strings(&["--stomme", "/bin/stomme", "--version", "1.2.3", "project"]);
    assert_eq!(parse_arguments(&missing), None);

    let full = strings(&[
        "--stomme",
        "/bin/stomme",
        "--version",
        "1.2.3",
        "--commit",
        COMMIT,
        "project",
    ]);

    assert_eq!(
        parse_arguments(&full),
        Some(Arguments {
            binary: PathBuf::from("/bin/stomme"),
            version: "1.2.3".to_string(),
            commit: COMMIT.to_string(),
            folder: PathBuf::from("project"),
        })
    );
}

#[test]
fn parse_arguments_rejects_a_commit_that_is_not_40_lowercase_hex() {
    let cases = [
        ("shorter all-hex", "0123456789abcdef0123456789abcdef0123456"),
        (
            "longer all-hex",
            "0123456789abcdef0123456789abcdef012345678",
        ),
        ("uppercase", "0123456789ABCDEF0123456789ABCDEF01234567"),
        (
            "non-hex at the end",
            "0123456789abcdef0123456789abcdef0123456g",
        ),
    ];

    for (name, commit) in cases {
        let arguments = strings(&[
            "--stomme",
            "/bin/stomme",
            "--version",
            "1.2.3",
            "--commit",
            commit,
            "project",
        ]);

        assert_eq!(parse_arguments(&arguments), None, "{name}: {commit}");
    }
}

#[test]
fn parse_arguments_rejects_each_misspelled_flag() {
    let cases = [
        (
            "stomme",
            strings(&[
                "--stomme-x",
                "/bin/stomme",
                "--version",
                "1.2.3",
                "--commit",
                COMMIT,
                "project",
            ]),
        ),
        (
            "version",
            strings(&[
                "--stomme",
                "/bin/stomme",
                "--version-x",
                "1.2.3",
                "--commit",
                COMMIT,
                "project",
            ]),
        ),
        (
            "commit",
            strings(&[
                "--stomme",
                "/bin/stomme",
                "--version",
                "1.2.3",
                "--commit-x",
                COMMIT,
                "project",
            ]),
        ),
    ];

    for (name, arguments) in cases {
        assert_eq!(parse_arguments(&arguments), None, "{name}");
    }
}
