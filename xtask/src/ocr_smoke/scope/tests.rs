use crate::ocr_smoke::{Preview, PreviewFile};

use super::selection_failures;

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
