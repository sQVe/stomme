use super::*;
use crate::test_support::strings;

fn status(report: &(Value, u8)) -> (Option<&str>, u8) {
    (report.0["status"].as_str(), report.1)
}

#[test]
fn report_value_carries_the_error_message() {
    assert_eq!(
        report_value("error", Some("broken"), &[], &[]),
        json!({
            "version": 1,
            "status": "error",
            "error": "broken",
            "survivors": [],
            "unmutated": [],
        })
    );
}

#[test]
fn ran_report_sets_status_and_exit_code() {
    let found = [Survivor {
        file: "src/lib.rs".to_string(),
        line: 3,
        mutator: "BinaryOperator".to_string(),
        original: "<=".to_string(),
        replacement: ">".to_string(),
    }];
    let skipped = strings(&["b.rs"]);

    let cases = [
        (Mode::Changed, &[][..], &[][..], "pass", 0, None),
        (Mode::Changed, &found[..], &[][..], "survivors", 2, None),
        (Mode::Audit, &found[..], &[][..], "survivors", 0, None),
        (
            Mode::Audit,
            &[][..],
            &skipped[..],
            "error",
            1,
            Some("Files over the mutant cap were not mutated."),
        ),
        (
            Mode::Changed,
            &found[..],
            &skipped[..],
            "survivors",
            1,
            None,
        ),
        (Mode::Audit, &found[..], &skipped[..], "survivors", 1, None),
    ];

    for (mode, survivors, unmutated, expected_status, exit, error) in cases {
        let actual = ran_report(mode, survivors, unmutated);
        let case = format!("{mode:?} {expected_status} {unmutated:?}");
        let expected_survivors = if survivors.is_empty() {
            json!([])
        } else {
            json!([{
                "file": "src/lib.rs", "line": 3, "mutator": "BinaryOperator",
                "original": "<=", "replacement": ">", "covered": true,
            }])
        };
        let mut expected = json!({
            "version": 1, "status": expected_status,
            "survivors": expected_survivors, "unmutated": unmutated,
        });
        if let Some(error) = error {
            expected["error"] = json!(error);
        }

        assert_eq!(status(&actual), (Some(expected_status), exit), "{case}");
        assert_eq!(actual.0, expected, "{case}");
        assert_eq!(actual.0.to_string(), expected.to_string(), "{case}");
    }
}
