use std::path::Path;
use std::process::ExitCode;

use super::*;
use crate::test_support::temporary_directory;

#[test]
fn report_renders_the_heading_and_each_outcome() {
    let folder = Path::new("/projects/example");
    let cases = [
        (Outcome::Passed, "### example\nPassed\n".to_string()),
        (
            Outcome::Failed {
                step: "pin",
                output: "stomme: x\n".to_string(),
            },
            "### example\nFailed at pin\n```text\nstomme: x\n```\n".to_string(),
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

#[test]
fn exit_code_maps_each_outcome() {
    let cases = [
        (Outcome::Passed, ExitCode::SUCCESS),
        (
            Outcome::Failed {
                step: "pin",
                output: String::new(),
            },
            ExitCode::FAILURE,
        ),
        (
            Outcome::NotAdopted {
                output: String::new(),
            },
            ExitCode::SUCCESS,
        ),
    ];

    for (outcome, expected) in cases {
        assert_eq!(exit_code(&outcome), expected, "{outcome:?}");
    }
}

#[test]
fn resolve_binary_finds_a_file_below_the_root() {
    let folder = temporary_directory();
    let binary = folder.path().join("stomme");
    std::fs::write(&binary, "").expect("write a binary");

    let resolved = resolve_binary(folder.path(), Path::new("stomme"));
    let expected = binary.canonicalize().expect("canonicalize the binary");

    assert_eq!(resolved, Some(expected));
}
