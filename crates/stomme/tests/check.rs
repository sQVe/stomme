use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../src/test_support.rs"]
mod test_support;

use test_support::{complete_project, write_file};

fn stomme(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stomme"))
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("run stomme")
}

fn stderr_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_string)
        .collect()
}

fn assert_single_problem(output: &Output, path: &str, rule: &str, case: &str) {
    let lines = stderr_lines(output);

    assert_eq!(output.status.code(), Some(1), "{case}");
    assert_eq!(lines.len(), 1, "{case}: {lines:?}");
    assert!(
        lines[0].starts_with(&format!("stomme: {path}: ")),
        "{case}: {lines:?}"
    );
    assert!(
        lines[0].ends_with(&format!(" [{rule}]")),
        "{case}: {lines:?}"
    );
}

#[test]
fn check_passes_a_complete_project() {
    let root = complete_project();

    let output = stomme(root.path(), &["check"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_lines(&output), Vec::<String>::new());
}

#[test]
fn check_reports_a_project_without_stomme_toml() {
    let root = complete_project();

    fs::remove_file(root.path().join("stomme.toml")).expect("remove stomme.toml");

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(&output, "stomme.toml", "no-stomme", "missing stomme.toml");
}

#[test]
fn check_reports_a_broken_stomme_toml() {
    let cases = [
        ("syntax error", "schema = 1\nkind = \"rust-cli\n"),
        (
            "unknown field",
            "schema = 1\nkind = \"rust-cli\"\nname = \"x\"\n",
        ),
        ("missing kind", "schema = 1\n"),
        ("schema 2", "schema = 2\nkind = \"rust-cli\"\n"),
    ];

    for (case, text) in cases {
        let root = complete_project();

        write_file(root.path(), "stomme.toml", text);

        let output = stomme(root.path(), &["check"]);

        assert_single_problem(&output, "stomme.toml", "broken-installation", case);
    }
}

#[test]
fn check_reports_every_problem_in_one_run() {
    let root = complete_project();

    fs::remove_file(root.path().join("stomme.toml")).expect("remove stomme.toml");
    fs::remove_file(root.path().join("LICENSE")).expect("remove LICENSE");
    write_file(
        root.path(),
        "docs/adr/0001-documentation-scope.md",
        "# Documentation scope\n",
    );

    let output = stomme(root.path(), &["check"]);
    let lines = stderr_lines(&output);
    let tails: Vec<&str> = lines
        .iter()
        .map(|line| line.rsplit_once(' ').map_or("", |(_, rule)| rule))
        .collect();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(tails, ["[no-stomme]", "[required-part]", "[adr-heading]"]);
}

#[test]
fn check_reports_an_adr_it_cannot_read() {
    let root = complete_project();
    let path = "docs/adr/0001-documentation-scope.md";

    fs::write(root.path().join(path), [0xff, 0xfe, b'\n']).expect("write invalid UTF-8");

    let output = stomme(root.path(), &["check"]);
    let lines = stderr_lines(&output);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("stomme: "), "{lines:?}");
    assert!(lines[0].contains(path), "{lines:?}");
}

#[test]
fn stomme_prints_usage_without_the_check_command() {
    let root = complete_project();
    let cases: [&[&str]; 3] = [&[], &["lint"], &["check", "extra"]];

    for arguments in cases {
        let output = stomme(root.path(), arguments);

        assert_eq!(output.status.code(), Some(1), "{arguments:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).starts_with("Usage: stomme check"),
            "{arguments:?}"
        );
    }
}
