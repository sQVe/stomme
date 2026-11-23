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

fn line(text: &str, number: usize) -> String {
    let found = text
        .lines()
        .nth(number - 1)
        .expect("the text has this line");

    found.to_string()
}

fn rendered_line(output: &str, marker: &str) -> String {
    let rest = output.split_once(marker).expect("the marker").1;
    let line = rest.lines().next().expect("the rendered line");

    line.to_string()
}

fn assert_pin_fails_at_line_12(expected: &str, found: &str) {
    let outcome = compare_workflow(found, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("Line 12"), "{output}");
    assert!(output.contains(&line(expected, 12)), "{output}");
    assert!(output.contains(&line(found, 12)), "{output}");
}

#[test]
fn release_pin_rejects_the_pin_inside_a_run_body() {
    let expected = expected_workflow(VERSION, COMMIT);
    let pin = line(&expected, 12);
    let run_step = format!("      - run: |\n          {}", pin.trim());
    let text = expected.replace(&pin, &run_step);

    let outcome = compare_workflow(&text, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("Line 12"), "{output}");
}

#[test]
fn release_pin_rejects_a_workflow_without_a_sync_job() {
    let expected = expected_workflow(VERSION, COMMIT);
    let text = expected.replace("  sync:", "  copy:");

    let outcome = compare_workflow(&text, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("Line 11"), "{output}");
}

#[test]
fn release_pin_rejects_a_pin_in_a_folded_scalar() {
    let expected = expected_workflow(VERSION, COMMIT);
    let pin = line(&expected, 12);
    let mut text = expected_workflow(VERSION, OTHER_COMMIT);
    let folded_scalar = format!(
        "  probe:\n    steps:\n      - run: >-\n          {}",
        pin.trim()
    );
    text.push_str(&folded_scalar);

    let outcome = compare_workflow(&text, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("Line 12"), "{output}");
}

#[test]
fn release_pin_passes_for_the_rendered_skeleton() {
    let text = expected_workflow(VERSION, COMMIT);

    assert_eq!(compare_workflow(&text, VERSION, COMMIT), Outcome::Passed);
}

#[test]
fn release_pin_fails_when_the_commit_differs() {
    let expected = expected_workflow(VERSION, COMMIT);
    let found = expected_workflow(VERSION, OTHER_COMMIT);

    assert_pin_fails_at_line_12(&expected, &found);
}

#[test]
fn release_pin_fails_for_the_tag_form() {
    let expected = expected_workflow(VERSION, COMMIT);
    let found = expected.replace(COMMIT, &format!("v{VERSION}"));

    assert_pin_fails_at_line_12(&expected, &found);
}

#[test]
fn release_pin_fails_when_the_version_comment_differs() {
    let expected = expected_workflow(VERSION, COMMIT);
    let found = expected.replace(&format!("# v{VERSION}"), "# v0.0.1");

    assert_pin_fails_at_line_12(&expected, &found);
}

#[test]
fn release_pin_reports_where_the_file_ends_first() {
    let expected = expected_workflow(VERSION, COMMIT);
    let found_short: String = expected.split_inclusive('\n').take(13).collect();
    let found_long = format!("{expected}  extra: line\n");
    let cases = [
        (
            "the found file ends first",
            found_short.as_str(),
            "The found file ends first.",
        ),
        (
            "the expected file ends first",
            found_long.as_str(),
            "The expected file ends first.",
        ),
    ];

    for (name, found, message) in cases {
        let outcome = compare_workflow(found, VERSION, COMMIT);

        let Outcome::Failed { step, output } = outcome else {
            panic!("{name}: expected a pin failure");
        };

        assert_eq!(step, "pin", "{name}");
        assert!(output.contains(message), "{name}: {output}");
    }
}

#[test]
fn release_pin_reports_a_missing_last_line_as_the_file_ending_first() {
    let expected = expected_workflow(VERSION, COMMIT);
    let last_line = format!("{}\n", line(&expected, 15));
    let found = expected
        .strip_suffix(&last_line)
        .expect("the skeleton ends with its last line");

    let outcome = compare_workflow(found, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("The found file ends first."), "{output}");
    assert!(!output.contains("Line 15"), "{output}");
}

#[test]
fn release_pin_reports_a_cut_inside_a_line_as_a_differing_line() {
    let expected = expected_workflow(VERSION, COMMIT);
    let last_line = format!("{}\n", line(&expected, 15));
    let cut = expected.len() - last_line.len() + 2;
    let found = &expected[..cut];

    let outcome = compare_workflow(found, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("Line 15"), "{output}");
    assert!(!output.contains("ends first"), "{output}");
}

#[test]
fn release_pin_rejects_a_crlf_workflow() {
    let expected = expected_workflow(VERSION, COMMIT);
    let found = expected.replace('\n', "\r\n");

    let outcome = compare_workflow(&found, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("Line 1"), "{output}");

    let expected_line = rendered_line(&output, "Expected:\n  ");
    let found_line = rendered_line(&output, "Found:\n  ");
    assert_ne!(expected_line, found_line, "{output}");
    assert!(found_line.contains("\\r"), "{output}");
}

#[test]
fn release_pin_rejects_a_workflow_without_the_final_newline() {
    let expected = expected_workflow(VERSION, COMMIT);
    let found = expected.trim_end_matches('\n');

    let outcome = compare_workflow(found, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("Line 15"), "{output}");

    let expected_line = rendered_line(&output, "Expected:\n  ");
    let found_line = rendered_line(&output, "Found:\n  ");
    assert_ne!(expected_line, found_line, "{output}");
}

#[test]
fn release_pin_shows_a_literal_backslash_n_in_the_differing_line() {
    let expected = expected_workflow(VERSION, COMMIT);
    let found = format!("{}\\n", expected.trim_end_matches('\n'));

    let outcome = compare_workflow(&found, VERSION, COMMIT);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a pin failure");
    };

    assert_eq!(step, "pin");
    assert!(output.contains("Line 15"), "{output}");

    let expected_line = rendered_line(&output, "Expected:\n  ");
    let found_line = rendered_line(&output, "Found:\n  ");
    assert_ne!(expected_line, found_line, "{output}");
}

#[test]
fn expected_workflow_leaves_no_placeholder() {
    let text = expected_workflow(VERSION, COMMIT);
    let pin =
        format!("    uses: sQVe/stomme/.github/workflows/sync-update.yml@{COMMIT} # v{VERSION}");

    assert!(!text.contains("{{commit}}"), "{text}");
    assert!(!text.contains("{{version}}"), "{text}");
    assert!(text.lines().any(|line| line == pin.as_str()), "{text}");
}

#[derive(Clone, Default)]
struct Fake {
    fails: bool,
    stderr: String,
}

thread_local! {
    static FAKE: RefCell<Fake> = RefCell::new(Fake::default());
    static READS: RefCell<usize> = const { RefCell::new(0) };
    static RUNS: RefCell<usize> = const { RefCell::new(0) };
}

fn install(fake: Fake) {
    FAKE.with(|installed| *installed.borrow_mut() = fake);
    READS.with(|reads| *reads.borrow_mut() = 0);
    RUNS.with(|runs| *runs.borrow_mut() = 0);
}

fn read_count() -> usize {
    READS.with(|reads| *reads.borrow())
}

fn run_count() -> usize {
    RUNS.with(|runs| *runs.borrow())
}

fn output(code: i32, stderr: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

fn fake_run(_binary: &Path, _folder: &Path) -> io::Result<Output> {
    RUNS.with(|runs| *runs.borrow_mut() += 1);

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

fn failed_directory_open(_folder: &Path) -> io::Result<Vec<io::Result<PathBuf>>> {
    Err(io::Error::new(io::ErrorKind::NotFound, "no such folder"))
}

fn failed_entry(_folder: &Path) -> io::Result<Vec<io::Result<PathBuf>>> {
    let error = io::Error::new(io::ErrorKind::PermissionDenied, "unreadable entry");

    Ok(vec![Err(error)])
}

fn runner_with_entries(entries: fn(&Path) -> io::Result<Vec<io::Result<PathBuf>>>) -> Runner {
    Runner {
        run: fake_run,
        read: counting_read,
        entries,
    }
}

fn runner() -> Runner {
    runner_with_entries(read_entries)
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
fn release_pin_rejects_a_folder_that_already_holds_a_file() {
    let folder = temporary_directory();
    let file = folder.path().join("keep.txt");
    std::fs::write(&file, "previous run").expect("write a file");
    install(Fake::default());

    let outcome = check_pin(
        &runner(),
        Path::new("/stomme"),
        VERSION,
        COMMIT,
        folder.path(),
    )
    .expect("run the check");

    assert_eq!(run_count(), 0);

    let contents = std::fs::read_to_string(&file).expect("read the file back");
    assert_eq!(contents, "previous run");

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a folder failure");
    };

    assert_eq!(step, "folder");
    assert!(
        output.contains(&folder.path().display().to_string()),
        "{output}"
    );
}

#[test]
fn release_pin_reports_a_folder_that_cannot_be_opened() {
    let folder = temporary_directory();
    install(Fake::default());

    let outcome = check_pin(
        &runner_with_entries(failed_directory_open),
        Path::new("/stomme"),
        VERSION,
        COMMIT,
        folder.path(),
    )
    .expect("run the check");

    assert_eq!(run_count(), 0);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a folder failure");
    };

    assert_eq!(step, "folder");
    assert!(output.contains("no such folder"), "{output}");
    assert!(
        output.contains(&folder.path().display().to_string()),
        "{output}"
    );
}

#[test]
fn release_pin_reports_a_folder_entry_that_cannot_be_read() {
    let folder = temporary_directory();
    install(Fake::default());

    let outcome = check_pin(
        &runner_with_entries(failed_entry),
        Path::new("/stomme"),
        VERSION,
        COMMIT,
        folder.path(),
    )
    .expect("run the check");

    assert_eq!(run_count(), 0);

    let Outcome::Failed { step, output } = outcome else {
        panic!("expected a folder failure");
    };

    assert_eq!(step, "folder");
    assert!(output.contains("unreadable entry"), "{output}");
    assert!(
        output.contains(&folder.path().display().to_string()),
        "{output}"
    );
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
