use std::ffi::OsString;

use super::*;

const EMBEDDED_COMMIT: &str = "1111111111111111111111111111111111111111";
const ENVIRONMENT_COMMIT: &str = "2222222222222222222222222222222222222222";

#[test]
fn embedded_commit_maps_the_build_value() {
    let cases = [("", None), (EMBEDDED_COMMIT, Some(EMBEDDED_COMMIT))];

    for (value, expected) in cases {
        assert_eq!(embedded(value), expected, "{value:?}");
    }
}

#[test]
fn release_commit_resolves_the_embedded_commit_then_the_environment() {
    let cases = [
        (
            "both",
            Some(EMBEDDED_COMMIT),
            Some(ENVIRONMENT_COMMIT),
            EMBEDDED_COMMIT,
        ),
        (
            "environment only",
            None,
            Some(ENVIRONMENT_COMMIT),
            ENVIRONMENT_COMMIT,
        ),
    ];

    for (name, embedded, environment, expected) in cases {
        let environment = environment.map(OsString::from);

        let commit = resolve(embedded, environment).expect("a commit resolves");

        assert_eq!(commit.0, expected, "{name}");
    }
}

#[test]
fn release_commit_refuses_without_a_commit() {
    let error = resolve(None, None).expect_err("no commit is known");

    assert!(
        matches!(&error, InitError::UnknownReleaseCommit { version } if *version == VERSION),
        "{error:?}"
    );
}

#[test]
fn release_commit_rejects_a_malformed_environment_commit() {
    let cases = [
        ("short", "a".repeat(39)),
        ("long", "a".repeat(41)),
        ("uppercase", "A".repeat(40)),
        ("not hex", "g".repeat(40)),
    ];

    for (name, value) in cases {
        let error = resolve(None, Some(OsString::from(value.as_str())))
            .expect_err("the commit is malformed");

        assert!(
            matches!(&error, InitError::InvalidReleaseCommit { value: bad } if *bad == value),
            "{name}: {error:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn release_commit_reports_a_non_utf8_environment_commit_lossily() {
    use std::os::unix::ffi::OsStringExt;

    let value = OsString::from_vec(vec![0xff; 40]);

    let error = resolve(None, Some(value)).expect_err("the commit is malformed");
    let expected = "\u{fffd}".repeat(40);

    assert!(
        matches!(&error, InitError::InvalidReleaseCommit { value } if *value == expected),
        "{error:?}"
    );
}
