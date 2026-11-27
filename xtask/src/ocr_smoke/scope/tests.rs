use crate::ocr_smoke::{Preview, PreviewFile};

use super::absence_failures;

#[test]
fn absence_fails_on_every_scope_outsider_in_the_preview() {
    let preview = Preview {
        files: vec![
            PreviewFile {
                path: "src/changed.rs".to_string(),
                will_review: true,
                exclude_reason: None,
            },
            PreviewFile {
                path: "src/kept.rs".to_string(),
                will_review: true,
                exclude_reason: None,
            },
            PreviewFile {
                path: "src/main_only.rs".to_string(),
                will_review: false,
                exclude_reason: Some("default_path".to_string()),
            },
        ],
    };

    let failures = absence_failures(&preview, &["src/kept.rs", "src/main_only.rs"]);

    assert_eq!(
        failures,
        [
            "ocr-smoke: scope: admission failed: src/kept.rs is in the preview",
            "ocr-smoke: scope: admission failed: src/main_only.rs is in the preview",
        ]
    );
}
