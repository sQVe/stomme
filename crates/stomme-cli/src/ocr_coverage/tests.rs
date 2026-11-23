use std::fs;
use std::path::Path;

use pulldown_cmark::{Event, Parser};
use serde_json::{Value, json};

use super::*;

enum Edit {
    Json(fn(&mut Value)),
    Bytes(fn(&[u8]) -> Vec<u8>),
}

enum Fragment {
    Whole(&'static str),
    Contains(&'static [&'static str]),
}

enum Expected {
    Coverage {
        exit_code: u8,
        reviewed: usize,
        unreviewed: &'static [(&'static str, &'static str, &'static str)],
        fragment: Fragment,
    },
    Error(fn(&OcrCoverageError) -> bool),
}

struct Case {
    name: &'static str,
    fixture: &'static str,
    edit: Edit,
    expected: Expected,
}

#[test]
fn classifier_cases_follow_one_decision_table() {
    let cases = [
        Case {
            name: "complete with an unrecorded gap is partial",
            fixture: "complete.json",
            edit: Edit::Json(remove_completed),
            expected: Expected::Coverage {
                exit_code: 2,
                reviewed: 0,
                unreviewed: &[("main.rs", "unknown", "no coverage record")],
                fragment: Fragment::Contains(&["- `main.rs`: `unknown`, `no coverage record`"]),
            },
        },
        Case {
            name: "a reused item counts as reviewed",
            fixture: "partial.json",
            edit: Edit::Json(move_failed_to_reused),
            expected: Expected::Coverage {
                exit_code: 0,
                reviewed: 6,
                unreviewed: &[],
                fragment: Fragment::Contains(&[]),
            },
        },
        Case {
            name: "a waived item is unreviewed and shows its reason",
            fixture: "complete.json",
            edit: Edit::Json(move_completed_to_waived),
            expected: Expected::Coverage {
                exit_code: 2,
                reviewed: 0,
                unreviewed: &[("main.rs", "waived", "accepted by a maintainer")],
                fragment: Fragment::Contains(&[
                    "- `main.rs`: `waived`, `accepted by a maintainer`",
                ]),
            },
        },
        Case {
            name: "a failed result with completed items still fails",
            fixture: "partial.json",
            edit: Edit::Json(mark_failed),
            expected: Expected::Coverage {
                exit_code: 1,
                reviewed: 5,
                unreviewed: &[(
                    "crates/stomme-cli/src/sync.rs",
                    "budget",
                    "reached the maximum tool-request rounds without finishing",
                )],
                fragment: Fragment::Contains(&[
                    "The OCR review failed.",
                    "- `crates/stomme-cli/src/sync.rs`: `budget`, `reached the maximum tool-request rounds without finishing`",
                ]),
            },
        },
        Case {
            name: "two unreviewed paths are sorted",
            fixture: "complete.json",
            edit: Edit::Json(add_two_unrecorded_selected_items),
            expected: Expected::Coverage {
                exit_code: 2,
                reviewed: 0,
                unreviewed: &[
                    ("a.rs", "unknown", "no coverage record"),
                    ("z.rs", "unknown", "no coverage record"),
                ],
                fragment: Fragment::Whole(concat!(
                    "### Review coverage\n",
                    "\n",
                    "OCR reviewed 0 of 2 selected files. The check fails until every selected file is reviewed.\n",
                    "\n",
                    "Not reviewed:\n",
                    "\n",
                    "- `a.rs`: `unknown`, `no coverage record`\n",
                    "- `z.rs`: `unknown`, `no coverage record`\n",
                )),
            },
        },
        Case {
            name: "a partial result with every item reviewed is inconsistent",
            fixture: "partial.json",
            edit: Edit::Json(mark_every_item_completed),
            expected: Expected::Error(|error| {
                matches!(error, OcrCoverageError::InconsistentPartial { .. })
            }),
        },
        Case {
            name: "a different schema version is unsupported",
            fixture: "complete.json",
            edit: Edit::Json(use_unsupported_schema),
            expected: Expected::Error(|error| {
                matches!(
                    error,
                    OcrCoverageError::UnsupportedSchema { schema_version, .. }
                        if schema_version == "ocr.run-manifest/v2"
                )
            }),
        },
        Case {
            name: "a status that differs from the terminal state is refused",
            fixture: "complete.json",
            edit: Edit::Json(mismatch_terminal_state),
            expected: Expected::Error(|error| {
                matches!(
                    error,
                    OcrCoverageError::StatusMismatch { status, terminal_state, .. }
                        if status == "complete" && terminal_state == "partial"
                )
            }),
        },
        Case {
            name: "an unknown status is refused",
            fixture: "complete.json",
            edit: Edit::Json(use_unknown_status),
            expected: Expected::Error(|error| {
                matches!(
                    error,
                    OcrCoverageError::UnknownStatus { status, .. } if status == "running"
                )
            }),
        },
        Case {
            name: "invalid JSON is refused",
            fixture: "complete.json",
            edit: Edit::Bytes(truncate_to_one_byte),
            expected: Expected::Error(|error| {
                matches!(error, OcrCoverageError::InvalidJson { .. })
            }),
        },
        Case {
            name: "a missing required field is refused",
            fixture: "complete.json",
            edit: Edit::Json(remove_manifest),
            expected: Expected::Error(|error| {
                matches!(error, OcrCoverageError::InvalidJson { .. })
            }),
        },
        Case {
            name: "unknown fields are accepted",
            fixture: "complete.json",
            edit: Edit::Json(add_unknown_fields),
            expected: Expected::Coverage {
                exit_code: 0,
                reviewed: 1,
                unreviewed: &[],
                fragment: Fragment::Contains(&[]),
            },
        },
    ];

    for case in cases {
        let bytes = fixture_bytes(case.fixture);
        let bytes = match case.edit {
            Edit::Json(edit) => {
                let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

                edit(&mut value);

                serde_json::to_vec(&value).expect("the edited result is JSON")
            }
            Edit::Bytes(edit) => edit(&bytes),
        };

        check(case.name, parse(&bytes, case.name), case.expected);
    }
}

#[test]
fn error_messages_escape_manifest_values_on_one_line() {
    let cases = [
        (
            "invalid JSON",
            OcrCoverageError::InvalidJson {
                path: "a\\b.json".to_string(),
                reason: "unexpected\ntoken".to_string(),
            },
            "a\\\\b.json is not a usable OCR result: unexpected\\ntoken",
        ),
        (
            "unsupported schema",
            OcrCoverageError::UnsupportedSchema {
                path: "a\nb.json".to_string(),
                schema_version: "ocr\r.run-manifest/v2".to_string(),
            },
            "a\\nb.json has unsupported schema ocr\\r.run-manifest/v2, expected ocr.run-manifest/v1",
        ),
        (
            "unknown status",
            OcrCoverageError::UnknownStatus {
                path: "a\u{85}b.json".to_string(),
                status: "run\tning".to_string(),
            },
            "a\\u{85}b.json has unknown status run\\tning",
        ),
        (
            "status mismatch",
            OcrCoverageError::StatusMismatch {
                path: "a\u{2028}b.json".to_string(),
                status: "complete".to_string(),
                terminal_state: "partial\u{202e}".to_string(),
            },
            "a\\u{2028}b.json status complete differs from manifest terminal state partial\\u{202e}",
        ),
        (
            "inconsistent partial",
            OcrCoverageError::InconsistentPartial {
                path: "a\tb.json".to_string(),
            },
            "a\\tb.json is partial but every selected item is reviewed",
        ),
    ];

    for (name, error, expected) in cases {
        assert_eq!(error.to_string(), expected, "{name}");
    }
}

#[test]
fn a_fragment_without_excluded_items_has_no_details_block() {
    let bytes = fixture_bytes("complete.json");
    let coverage = parse(&bytes, "complete.json").expect("parse the complete result");
    let fragment = coverage.fragment();

    assert_eq!(
        fragment,
        "### Review coverage\n\nOCR reviewed 1 of 1 selected file.\n"
    );
}

#[test]
fn a_padded_path_keeps_its_spaces_in_the_fragment() {
    let bytes = fixture_bytes("skipped.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["excluded"][0]["path"] = json!(" padded ");

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "skipped.json").expect("parse the edited result");

    assert_eq!(
        coverage.fragment(),
        concat!(
            "### Review coverage\n",
            "\n",
            "OCR selected no files to review.\n",
            "\n",
            "<details>\n",
            "<summary>1 file excluded by the review rules</summary>\n",
            "\n",
            "- `  padded  `: `user_exclude`\n",
            "</details>\n",
        )
    );
}

#[test]
fn a_path_with_a_line_break_is_escaped_in_the_fragment_and_the_unreviewed_lines() {
    let bytes = fixture_bytes("complete.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["completed"] = json!([]);
    value["manifest"]["coverage"]["selected"][0]["path"] = json!("x\n ");

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "complete.json").expect("parse the edited result");

    assert_eq!(
        coverage.fragment(),
        concat!(
            "### Review coverage\n",
            "\n",
            "OCR reviewed 0 of 1 selected file. The check fails until every selected file is reviewed.\n",
            "\n",
            "Not reviewed:\n",
            "\n",
            "- ` x\\n  `: `unknown`, `no coverage record`\n",
        )
    );
    assert_eq!(
        coverage.unreviewed_lines(),
        vec!["x\\n : not reviewed (unknown: no coverage record)"]
    );
}

#[test]
fn a_newline_path_and_a_literal_backslash_n_path_get_different_bullets() {
    let bytes = fixture_bytes("skipped.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["excluded"] = json!([
        { "item_id": "line", "path": "x\n ", "exclude_reason": "user_exclude" },
        { "item_id": "text", "path": "x\\n ", "exclude_reason": "user_exclude" },
    ]);

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "skipped.json").expect("parse the edited result");

    assert_eq!(
        coverage.fragment(),
        concat!(
            "### Review coverage\n",
            "\n",
            "OCR selected no files to review.\n",
            "\n",
            "<details>\n",
            "<summary>2 files excluded by the review rules</summary>\n",
            "\n",
            "- ` x\\n  `: `user_exclude`\n",
            "- ` x\\\\n  `: `user_exclude`\n",
            "</details>\n",
        )
    );
}

#[test]
fn a_classification_and_reason_with_a_line_break_render_on_one_line() {
    let bytes = fixture_bytes("failed.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["failed"][0]["classification"] = json!("provider\n");
    value["manifest"]["coverage"]["failed"][0]["reason"] = json!("failed\nreason");

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "failed.json").expect("parse the edited result");

    assert_eq!(
        coverage.fragment(),
        concat!(
            "### Review coverage\n",
            "\n",
            "The OCR review failed. OCR reviewed 0 of 1 selected file.\n",
            "\n",
            "Not reviewed:\n",
            "\n",
            "- `main.rs`: `provider\\n`, `failed\\nreason`\n",
        )
    );
    assert_eq!(
        coverage.unreviewed_lines(),
        vec!["main.rs: not reviewed (provider\\n: failed\\nreason)"]
    );
}

#[test]
fn an_excluded_reason_with_a_line_break_is_escaped_in_the_details_block() {
    let bytes = fixture_bytes("skipped.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["excluded"][0]["exclude_reason"] = json!("user\nexclude");

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "skipped.json").expect("parse the edited result");

    assert_eq!(
        coverage.fragment(),
        concat!(
            "### Review coverage\n",
            "\n",
            "OCR selected no files to review.\n",
            "\n",
            "<details>\n",
            "<summary>1 file excluded by the review rules</summary>\n",
            "\n",
            "- `main.rs`: `user\\nexclude`\n",
            "</details>\n",
        )
    );
}

#[test]
fn a_reason_with_a_newline_and_a_literal_backslash_n_render_differently() {
    let newline = failed_fragment_with_reason("a\nb");
    let literal = failed_fragment_with_reason("a\\nb");

    assert_eq!(bullet(&newline), "- `main.rs`: `provider`, `a\\nb`");
    assert_eq!(bullet(&literal), "- `main.rs`: `provider`, `a\\\\nb`");
}

#[test]
fn markdown_in_a_reason_stays_literal_inside_a_code_span() {
    let cases = [
        (
            "Expected **literal** stars",
            "- `main.rs`: `provider`, `Expected **literal** stars`",
        ),
        ("<b>x</b>", "- `main.rs`: `provider`, `<b>x</b>`"),
        ("@user", "- `main.rs`: `provider`, `@user`"),
        ("#51", "- `main.rs`: `provider`, `#51`"),
    ];

    for (reason, expected) in cases {
        let fragment = failed_fragment_with_reason(reason);

        assert_eq!(bullet(&fragment), expected, "{reason}");
    }
}

#[test]
fn a_backtick_in_a_reason_widens_the_code_span() {
    let fragment = failed_fragment_with_reason("a `b` c");

    assert_eq!(bullet(&fragment), "- `main.rs`: `provider`, ``a `b` c``");
}

#[test]
fn an_empty_classification_renders_as_plain_empty_text() {
    let bytes = fixture_bytes("failed.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["failed"][0]["classification"] = json!("");
    value["manifest"]["coverage"]["failed"][0]["reason"] = json!("bad `token`");

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "failed.json").expect("parse the edited result");

    assert_eq!(
        bullet(&coverage.fragment()),
        "- `main.rs`: (empty), `` bad `token` ``"
    );
}

#[test]
fn an_empty_path_renders_as_plain_empty_text_and_keeps_its_stderr_line() {
    let bytes = fixture_bytes("complete.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["selected"][0]["path"] = json!("");
    value["manifest"]["coverage"]["completed"] = json!([]);

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "complete.json").expect("parse the edited result");

    assert_eq!(coverage.exit_code(), 2);
    assert_eq!(
        bullet(&coverage.fragment()),
        "- (empty): `unknown`, `no coverage record`"
    );
    assert_eq!(
        coverage.unreviewed_lines(),
        vec![": not reviewed (unknown: no coverage record)"]
    );
}

#[test]
fn an_empty_reason_renders_as_plain_empty_text() {
    let failed = failed_fragment_with_reason("");

    assert_eq!(bullet(&failed), "- `main.rs`: `provider`, (empty)");

    let waived = waived_fragment_with_reason("");

    assert_eq!(bullet(&waived), "- `main.rs`: `waived`, (empty)");
}

#[test]
fn an_empty_exclude_reason_renders_as_plain_empty_text() {
    let bytes = fixture_bytes("skipped.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["excluded"][0]["path"] = json!("vendor/x.js");
    value["manifest"]["coverage"]["excluded"][0]["exclude_reason"] = json!("");

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "skipped.json").expect("parse the edited result");

    assert_eq!(
        coverage.fragment(),
        concat!(
            "### Review coverage\n",
            "\n",
            "OCR selected no files to review.\n",
            "\n",
            "<details>\n",
            "<summary>1 file excluded by the review rules</summary>\n",
            "\n",
            "- `vendor/x.js`: (empty)\n",
            "</details>\n",
        )
    );
}

#[test]
fn a_literal_empty_marker_renders_as_a_code_span() {
    let fragment = failed_fragment_with_reason("(empty)");

    assert_eq!(bullet(&fragment), "- `main.rs`: `provider`, `(empty)`");
}

#[test]
fn the_probe_bullet_parses_into_whole_code_spans_and_plain_empty_text() {
    let bytes = fixture_bytes("failed.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["failed"][0]["classification"] = json!("");
    value["manifest"]["coverage"]["failed"][0]["reason"] = json!("bad `token`");

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "failed.json").expect("parse the edited result");
    let fragment = coverage.fragment();
    let bullet = bullet(&fragment);
    let mut code_spans = Vec::new();
    let mut plain_text = String::new();

    for event in Parser::new(bullet) {
        match event {
            Event::Code(text) => code_spans.push(text.to_string()),
            Event::Text(text) => plain_text.push_str(&text),
            _ => {}
        }
    }

    assert_eq!(code_spans, vec!["main.rs", "bad `token`"]);
    assert_eq!(plain_text, ": (empty), ");
}

#[test]
fn escaped_text_escapes_only_the_display_breaking_characters() {
    let bidi = [
        '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}',
        '\u{2068}', '\u{2069}', '\u{200e}', '\u{200f}', '\u{061c}',
    ];

    for character in bidi {
        let text = format!("x{character}y");
        let expected = format!("x\\u{{{:x}}}y", character as u32);

        assert_eq!(escaped_text(&text), expected, "U+{:04X}", character as u32);
    }

    let cases = [
        ("a literal backslash before n", "x\\n", "x\\\\n"),
        ("a line break", "x\n", "x\\n"),
        ("a carriage return", "x\r", "x\\r"),
        ("a tab", "x\t", "x\\t"),
        ("a control character", "x\u{1}y", "x\\u{1}y"),
        ("a C1 control character", "x\u{85}y", "x\\u{85}y"),
        ("a delete character", "x\u{7f}y", "x\\u{7f}y"),
        ("a mixed string", "a\tb\rc\u{1}d", "a\\tb\\rc\\u{1}d"),
        ("a line separator", "x\u{2028}y", "x\\u{2028}y"),
        ("a paragraph separator", "x\u{2029}y", "x\\u{2029}y"),
        (
            "a zero width non-joiner",
            "می\u{200c}خواهم.md",
            "می\u{200c}خواهم.md",
        ),
        (
            "a zero width joiner in an emoji",
            "\u{1f468}\u{200d}\u{1f469}",
            "\u{1f468}\u{200d}\u{1f469}",
        ),
        ("a byte order mark", "x\u{feff}y", "x\u{feff}y"),
    ];

    for (name, text, expected) in cases {
        assert_eq!(escaped_text(text), expected, "{name}");
    }
}

#[test]
fn code_span_keeps_padding_and_fence_width() {
    assert_eq!(code_span("src/a`b.rs"), "``src/a`b.rs``");
    assert_eq!(code_span("`src/a.rs"), "`` `src/a.rs ``");
    assert_eq!(code_span("src/a.rs`"), "`` src/a.rs` ``");
    assert_eq!(code_span(" padded "), "`  padded  `");
    assert_eq!(code_span(" padded"), "`  padded `");
    assert_eq!(code_span("padded "), "` padded  `");
    assert_eq!(code_span("   "), "`   `");
}

fn failed_fragment_with_reason(reason: &str) -> String {
    let bytes = fixture_bytes("failed.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    value["manifest"]["coverage"]["failed"][0]["reason"] = json!(reason);

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "failed.json").expect("parse the edited result");

    coverage.fragment()
}

fn waived_fragment_with_reason(reason: &str) -> String {
    let bytes = fixture_bytes("complete.json");
    let mut value: Value = serde_json::from_slice(&bytes).expect("the fixture is JSON");

    move_completed_to_waived(&mut value);
    value["manifest"]["coverage"]["waived"][0]["reason"] = json!(reason);

    let edited = serde_json::to_vec(&value).expect("the edited result is JSON");
    let coverage = parse(&edited, "complete.json").expect("parse the edited result");

    coverage.fragment()
}

fn bullet(fragment: &str) -> &str {
    fragment
        .lines()
        .find(|line| line.starts_with("- "))
        .expect("the fragment has a bullet")
}

fn check(name: &str, result: Result<Coverage, OcrCoverageError>, expected: Expected) {
    match expected {
        Expected::Coverage {
            exit_code,
            reviewed,
            unreviewed,
            fragment: expected_fragment,
        } => {
            let coverage = result.unwrap_or_else(|error| panic!("{name}: {error}"));
            let actual: Vec<(&str, &str, &str)> = coverage
                .unreviewed
                .iter()
                .map(|item| {
                    (
                        item.path.as_str(),
                        item.classification.as_str(),
                        item.reason.as_str(),
                    )
                })
                .collect();

            assert_eq!(coverage.exit_code(), exit_code, "{name}");
            assert_eq!(coverage.reviewed, reviewed, "{name}");
            assert_eq!(actual, unreviewed, "{name}");

            let fragment = coverage.fragment();

            match expected_fragment {
                Fragment::Whole(expected) => assert_eq!(fragment, expected, "{name}"),
                Fragment::Contains(expected_texts) => {
                    for expected_text in expected_texts {
                        assert!(fragment.contains(expected_text), "{name}: {fragment}");
                    }
                }
            }
        }
        Expected::Error(matches) => {
            let error = result.expect_err(name);

            assert!(matches(&error), "{name}: {error:?}");
        }
    }
}

fn remove_completed(value: &mut Value) {
    value["manifest"]["coverage"]["completed"] = json!([]);
}

fn move_failed_to_reused(value: &mut Value) {
    let failed = value["manifest"]["coverage"]["failed"].take();

    value["manifest"]["coverage"]["reused"] = failed;
    value["manifest"]["coverage"]["failed"] = json!([]);
    value["status"] = json!("complete");
    value["manifest"]["terminal_state"] = json!("complete");
}

fn move_completed_to_waived(value: &mut Value) {
    let completed = value["manifest"]["coverage"]["completed"].take();
    let mut item = completed[0].clone();

    item["reason"] = json!("accepted by a maintainer");
    value["manifest"]["coverage"]["completed"] = json!([]);
    value["manifest"]["coverage"]["waived"] = json!([item]);
}

fn mark_failed(value: &mut Value) {
    value["status"] = json!("failed");
    value["manifest"]["terminal_state"] = json!("failed");
}

fn add_two_unrecorded_selected_items(value: &mut Value) {
    value["manifest"]["coverage"]["selected"] = json!([
        { "item_id": "z", "path": "z.rs" },
        { "item_id": "a", "path": "a.rs" },
    ]);
    value["manifest"]["coverage"]["completed"] = json!([]);
}

fn mark_every_item_completed(value: &mut Value) {
    value["manifest"]["coverage"]["completed"] = value["manifest"]["coverage"]["selected"].clone();
    value["manifest"]["coverage"]["failed"] = json!([]);
}

fn use_unsupported_schema(value: &mut Value) {
    value["manifest"]["schema_version"] = json!("ocr.run-manifest/v2");
}

fn mismatch_terminal_state(value: &mut Value) {
    value["manifest"]["terminal_state"] = json!("partial");
}

fn use_unknown_status(value: &mut Value) {
    value["status"] = json!("running");
}

fn truncate_to_one_byte(bytes: &[u8]) -> Vec<u8> {
    let mut bytes = bytes.to_vec();

    bytes.truncate(1);

    bytes
}

fn remove_manifest(value: &mut Value) {
    value
        .as_object_mut()
        .expect("the result is an object")
        .remove("manifest");
}

fn add_unknown_fields(value: &mut Value) {
    value["extra"] = json!(true);
    value["manifest"]["extra"] = json!({});
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ocr-results")
        .join(name);

    fs::read(path).expect("read the fixture")
}
