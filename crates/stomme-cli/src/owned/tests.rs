use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

fn foundation_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../foundation")
}

fn collect_files(root: &Path, folder: &Path, files: &mut BTreeSet<String>) {
    for entry in fs::read_dir(root.join(folder)).expect("read a source folder") {
        let entry = entry.expect("read a source entry");
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("a source file lies inside the foundation folder")
            .to_string_lossy()
            .replace('\\', "/");

        if entry.file_type().expect("read the file type").is_dir() {
            collect_files(root, Path::new(&relative), files);
        } else {
            files.insert(relative);
        }
    }
}

fn source_files() -> BTreeSet<String> {
    let root = foundation_root();
    let mut files = BTreeSet::new();

    collect_files(&root, Path::new("shared"), &mut files);

    for language in fs::read_dir(root.join("languages")).expect("read the languages folder") {
        let language = language.expect("read a language entry");
        let name = language.file_name().to_string_lossy().into_owned();

        files.insert(format!("languages/{name}/guide.md"));
    }

    for layer in fs::read_dir(root.join("layers")).expect("read the layers folder") {
        let layer = layer.expect("read a layer entry");
        let name = layer.file_name().to_string_lossy().into_owned();

        files.insert(format!("layers/{name}/guide.md"));
    }

    files
}

fn file(path: &str, content: &'static str) -> OwnedFile {
    OwnedFile {
        path: path.to_string(),
        content,
    }
}

#[test]
fn embedded_sources_match_the_source_tree() {
    assert_eq!(embedded_paths(), source_files());
}

#[test]
fn validate_links_accepts_a_link_inside_the_set() {
    let files = [
        file("docs/stomme/shared/one.md", "[two](./two.md#part)\n"),
        file("docs/stomme/shared/two.md", "# Two\n\n## Part\n"),
    ];

    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn validate_links_reports_a_link_outside_the_set() {
    let files = [file(
        "docs/stomme/shared/one.md",
        "[missing](./missing.md)\n",
    )];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::Broken {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./missing.md".to_string(),
        })
    );
}

#[test]
fn validate_links_ignores_links_in_code_and_comments() {
    let files = [file(
        "docs/stomme/shared/one.md",
        "`[a](./missing.md)`\n\n```\n[b](./missing.md)\n```\n\n<!-- [c](./missing.md) -->\n",
    )];

    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn validate_links_reports_a_link_after_a_closed_code_span() {
    let files = [file(
        "docs/stomme/shared/one.md",
        "`[inside](./inside.md)`\n[after](./after.md)\n",
    )];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::Broken {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./after.md".to_string(),
        })
    );
}

#[test]
fn validate_links_reports_a_link_after_a_closed_fence() {
    let files = [file(
        "docs/stomme/shared/one.md",
        "```\n[inside](./inside.md)\n```\n[after](./after.md)\n",
    )];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::Broken {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./after.md".to_string(),
        })
    );
}

#[test]
fn validate_links_ignores_a_link_inside_a_mismatched_code_span() {
    let files = [file(
        "docs/stomme/shared/one.md",
        "`` `[inside](./inside.md)` ``\n[after](./after.md)\n",
    )];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::Broken {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./after.md".to_string(),
        })
    );
}

#[test]
fn validate_links_reports_a_missing_anchor() {
    let files = [
        file("docs/stomme/shared/one.md", "[two](./two.md#missing)\n"),
        file("docs/stomme/shared/two.md", "## Test placement\n"),
    ];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::MissingAnchor {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./two.md#missing".to_string(),
            anchor: "missing".to_string(),
            resolved: "docs/stomme/shared/two.md".to_string(),
        })
    );
}

#[test]
fn validate_links_accepts_a_heading_slug_anchor() {
    let files = [
        file(
            "docs/stomme/shared/one.md",
            "[two](./two.md#test-placement)\n",
        ),
        file("docs/stomme/shared/two.md", "## Test placement\n"),
    ];

    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn validate_links_numbers_duplicate_heading_anchors() {
    let files = [
        file(
            "docs/stomme/shared/one.md",
            "[a](./two.md#test-placement) [b](./two.md#test-placement-1) [c](./two.md#test-placement-2)\n",
        ),
        file(
            "docs/stomme/shared/two.md",
            "# Test placement\n## Test placement\n### Test placement\n",
        ),
    ];

    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn validate_links_reports_a_number_past_the_duplicate_headings() {
    let files = [
        file(
            "docs/stomme/shared/one.md",
            "[a](./two.md#test-placement-2)\n",
        ),
        file(
            "docs/stomme/shared/two.md",
            "# Test placement\n## Test placement\n",
        ),
    ];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::MissingAnchor {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./two.md#test-placement-2".to_string(),
            anchor: "test-placement-2".to_string(),
            resolved: "docs/stomme/shared/two.md".to_string(),
        })
    );
}

#[test]
fn validate_links_ignores_a_heading_inside_a_fence() {
    let files = [
        file("docs/stomme/shared/one.md", "[two](./two.md#hidden)\n"),
        file("docs/stomme/shared/two.md", "```\n## Hidden\n```\n"),
    ];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::MissingAnchor {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./two.md#hidden".to_string(),
            anchor: "hidden".to_string(),
            resolved: "docs/stomme/shared/two.md".to_string(),
        })
    );
}

#[test]
fn validate_links_ignores_a_heading_inside_a_mismatched_fence() {
    let files = [
        file("docs/stomme/shared/one.md", "[two](./two.md#also-hidden)\n"),
        file(
            "docs/stomme/shared/two.md",
            "~~~\n## Hidden\n```\n## Also hidden\n~~~\n",
        ),
    ];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::MissingAnchor {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./two.md#also-hidden".to_string(),
            anchor: "also-hidden".to_string(),
            resolved: "docs/stomme/shared/two.md".to_string(),
        })
    );
}

#[test]
fn validate_links_ignores_a_heading_after_a_short_closing_fence() {
    let files = [
        file("docs/stomme/shared/one.md", "[two](./two.md#also-hidden)\n"),
        file(
            "docs/stomme/shared/two.md",
            "````\n## Hidden\n```\n## Also hidden\n````\n",
        ),
    ];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::MissingAnchor {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./two.md#also-hidden".to_string(),
            anchor: "also-hidden".to_string(),
            resolved: "docs/stomme/shared/two.md".to_string(),
        })
    );
}

#[test]
fn validate_links_accepts_a_heading_indented_by_three_spaces() {
    let files = [
        file(
            "docs/stomme/shared/one.md",
            "[two](./two.md#test-placement)\n",
        ),
        file("docs/stomme/shared/two.md", "   ## Test placement\n"),
    ];

    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn validate_links_reports_a_heading_hash_without_a_space() {
    let files = [
        file(
            "docs/stomme/shared/one.md",
            "[two](./two.md#test-placement)\n",
        ),
        file("docs/stomme/shared/two.md", "##Test placement\n"),
    ];

    assert_eq!(
        validate_links(&files),
        Err(LinkError::MissingAnchor {
            path: "docs/stomme/shared/one.md".to_string(),
            target: "./two.md#test-placement".to_string(),
            anchor: "test-placement".to_string(),
            resolved: "docs/stomme/shared/two.md".to_string(),
        })
    );
}

#[test]
fn validate_links_accepts_a_closing_hash_sequence() {
    let files = [
        file(
            "docs/stomme/shared/one.md",
            "[two](./two.md#test-placement)\n",
        ),
        file("docs/stomme/shared/two.md", "## Test placement ##\n"),
    ];

    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn validate_links_drops_punctuation_from_anchors() {
    let files = [
        file(
            "docs/stomme/shared/one.md",
            "[two](./two.md#test-placement-done)\n",
        ),
        file("docs/stomme/shared/two.md", "## Test placement, done!\n"),
    ];

    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn validate_links_ignores_external_and_anchor_links() {
    let files = [file(
        "docs/stomme/shared/one.md",
        "[a](https://example.com) [b](#part) [c](/absolute.md)\n",
    )];

    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn fences_indented_by_three_or_fewer_spaces_hide_links() {
    for spaces in 0..=3 {
        let indent = " ".repeat(spaces);
        let text = format!("{indent}~~~\n[inside](./missing.md)\n{indent}~~~\n");

        assert_eq!(link_targets(&text), Vec::<String>::new(), "indent {spaces}");
    }
}

#[test]
fn a_fence_indented_by_four_spaces_does_not_hide_links() {
    let text = "    ~~~\n[inside](./missing.md)\n    ~~~\n";

    assert_eq!(link_targets(text), vec!["./missing.md"]);
}

#[test]
fn tilde_fences_hide_links() {
    let text = "~~~\n[inside](./missing.md)\n~~~\n";

    assert_eq!(link_targets(text), Vec::<String>::new());
}

#[test]
fn fewer_than_three_fence_markers_do_not_hide_links() {
    for markers in ["~", "~~"] {
        let text = format!("{markers}\n[inside](./missing.md)\n{markers}\n");

        assert_eq!(link_targets(&text), vec!["./missing.md"], "{markers}");
    }
}

#[test]
fn a_run_of_three_dashes_does_not_open_a_fence() {
    let text = "---\n[after](./after.md)\n---\n";

    assert_eq!(link_targets(text), vec!["./after.md"]);
}

#[test]
fn a_closing_fence_needs_the_same_marker() {
    let text = "~~~\n[inside](./inside.md)\n```\n[after](./after.md)\n~~~\n";

    assert_eq!(link_targets(text), Vec::<String>::new());
}

#[test]
fn a_closing_fence_needs_at_least_the_opening_length() {
    let text = "````\n[inside](./inside.md)\n```\n[after](./after.md)\n````\n";

    assert_eq!(link_targets(text), Vec::<String>::new());
}

#[test]
fn owned_set_reads_the_given_catalog() {
    let catalog = crate::layer::test_catalog(&[("one", "language = \"rust\"\n")]);
    let selection = Selection {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language: crate::layer::Language::Rust,
        layers: BTreeSet::from(["one".to_string()]),
        roles: BTreeMap::new(),
        violations: Vec::new(),
        extra_docs: Vec::new(),
        modules: BTreeMap::new(),
    };

    let files = owned_set(&selection, &catalog);

    assert!(
        files
            .iter()
            .any(|file| file.path == "docs/stomme/layers/one/guide.md")
    );
}

#[test]
fn every_layer_passes_links_in_its_smallest_selection() {
    let catalog = crate::layer::catalog();

    for (name, _) in catalog.guides() {
        let (language, layers) = catalog
            .smallest_selection(name)
            .expect("a registered layer has a language and requirements");
        let selection = Selection {
            version: env!("CARGO_PKG_VERSION").to_string(),
            language,
            layers,
            roles: BTreeMap::new(),
            violations: Vec::new(),
            extra_docs: Vec::new(),
            modules: BTreeMap::new(),
        };
        let files = owned_set(&selection, catalog);

        assert_eq!(
            validate_links(&files),
            Ok(()),
            "layer `{name}` fails links in its smallest selection"
        );
    }
}

#[test]
fn bundle_set_holds_every_language_and_layer() {
    let catalog = crate::layer::catalog();
    let files = bundle_set(catalog);
    let paths: BTreeSet<&str> = files.iter().map(|file| file.path.as_str()).collect();

    assert!(paths.contains("docs/stomme/shared/coding.md"));
    assert!(paths.contains("docs/stomme/languages/rust/guide.md"));
    assert!(paths.contains("docs/stomme/languages/typescript/guide.md"));
    assert!(paths.contains("docs/stomme/layers/herdr/guide.md"));
    assert_eq!(paths.len(), embedded_paths().len());
}
