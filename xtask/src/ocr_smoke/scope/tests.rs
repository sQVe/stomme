use crate::ocr_smoke::{Preview, PreviewFile};

use super::{scope_base, selection_failures};

#[test]
fn scope_base_rejects_the_empty_sentinel() {
    let error = scope_base(
        "empty\n",
        "stomme: scope: 0 files, 0 added lines, 0 removed lines\n",
    )
    .expect_err("reject the empty sentinel");

    assert_eq!(
        format!("{error:#}"),
        "ocr-smoke: scope: ocr-scope found no unreviewed change; stderr:\nstomme: scope: 0 files, 0 added lines, 0 removed lines"
    );
}

#[test]
fn scope_base_rejects_blank_stdout() {
    let error = scope_base(
        "\n",
        "stomme: scope: 0 files, 0 added lines, 0 removed lines\n",
    )
    .expect_err("reject blank stdout");

    assert_eq!(
        format!("{error:#}"),
        "ocr-smoke: scope: ocr-scope printed no base commit; stderr:\nstomme: scope: 0 files, 0 added lines, 0 removed lines"
    );
}

#[test]
fn scope_base_trims_the_printed_commit() {
    assert_eq!(
        scope_base(" abc123 \n", "").expect("read the base"),
        "abc123"
    );
}

#[test]
fn selection_reports_every_mismatch() {
    let extra = preview(&[("src/changed.rs", true), ("src/unexpected.rs", true)]);
    let missing = preview(&[("src/changed.rs", false)]);
    let cases = [
        (
            "the expected files",
            preview(&[("src/changed.rs", true), ("src/main_only.rs", false)]),
            Vec::<String>::new(),
        ),
        (
            "an unexpected extra path",
            extra,
            vec![
                "ocr-smoke: scope: admission failed: the preview selected [\"src/changed.rs\", \"src/unexpected.rs\"], expected [\"src/changed.rs\"]"
                    .to_string(),
            ],
        ),
        (
            "a missing path",
            missing,
            vec![
                "ocr-smoke: scope: admission failed: the preview selected [], expected [\"src/changed.rs\"]"
                    .to_string(),
            ],
        ),
    ];

    for (name, preview, expected) in cases {
        assert_eq!(
            selection_failures(&preview, &["src/changed.rs"]),
            expected,
            "{name}"
        );
    }
}

fn preview(files: &[(&str, bool)]) -> Preview {
    Preview {
        files: files
            .iter()
            .map(|(path, will_review)| PreviewFile {
                path: path.to_string(),
                will_review: *will_review,
                exclude_reason: None,
            })
            .collect(),
    }
}
