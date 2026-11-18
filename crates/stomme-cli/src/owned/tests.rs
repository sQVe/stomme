use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::layer::{Catalog, SelectionError};

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

struct AnchorCase {
    name: &'static str,
    headings: &'static str,
    links: &'static str,
    first_missing: Option<&'static str>,
}

#[test]
fn validate_links_decides_each_heading_anchor() {
    let cases = [
        AnchorCase {
            name: "a slug of the heading text",
            headings: "## Test placement\n",
            links: "[two](./two.md#test-placement)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "an anchor no heading has",
            headings: "## Test placement\n",
            links: "[two](./two.md#missing)\n",
            first_missing: Some("missing"),
        },
        AnchorCase {
            name: "a heading with an underscore",
            headings: "## foo_bar\n",
            links: "[two](./two.md#foo_bar)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "an empty fragment",
            headings: "# Two\n",
            links: "[two](./two.md#)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "the first duplicate heading",
            headings: "# Test placement\n## Test placement\n### Test placement\n",
            links: "[two](./two.md#test-placement)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "the second duplicate heading",
            headings: "# Test placement\n## Test placement\n### Test placement\n",
            links: "[two](./two.md#test-placement-1)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "the third duplicate heading",
            headings: "# Test placement\n## Test placement\n### Test placement\n",
            links: "[two](./two.md#test-placement-2)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a number past the duplicate headings",
            headings: "# Test placement\n## Test placement\n",
            links: "[two](./two.md#test-placement-2)\n",
            first_missing: Some("test-placement-2"),
        },
        AnchorCase {
            name: "a suffix a later heading already uses",
            headings: "# Setup\n# Setup-1\n# Setup\n",
            links: "[two](./two.md#setup-2)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "the plain slug before a used suffix",
            headings: "# Setup\n# Setup-1\n# Setup\n",
            links: "[two](./two.md#setup)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "the written suffix before a used suffix",
            headings: "# Setup\n# Setup-1\n# Setup\n",
            links: "[two](./two.md#setup-1)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a heading inside a fence",
            headings: "```\n## Hidden\n```\n",
            links: "[two](./two.md#hidden)\n",
            first_missing: Some("hidden"),
        },
        AnchorCase {
            name: "a heading inside a mismatched fence",
            headings: "~~~\n## Hidden\n```\n## Also hidden\n~~~\n",
            links: "[two](./two.md#also-hidden)\n",
            first_missing: Some("also-hidden"),
        },
        AnchorCase {
            name: "a heading after a short closing fence",
            headings: "````\n## Hidden\n```\n## Also hidden\n````\n",
            links: "[two](./two.md#also-hidden)\n",
            first_missing: Some("also-hidden"),
        },
        AnchorCase {
            name: "a heading after a fence line with trailing text",
            headings: "```\n```not a closer\n## Hidden\n```\n",
            links: "[two](./two.md#hidden)\n",
            first_missing: Some("hidden"),
        },
        AnchorCase {
            name: "a heading after a closed fence",
            headings: "```\n## Inside\n```\n## After\n",
            links: "[two](./two.md#after)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a heading indented by three spaces",
            headings: "   ## Test placement\n",
            links: "[two](./two.md#test-placement)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a heading hash without a space",
            headings: "##Test placement\n",
            links: "[two](./two.md#test-placement)\n",
            first_missing: Some("test-placement"),
        },
        AnchorCase {
            name: "a heading with a closing hash sequence",
            headings: "## Test placement ##\n",
            links: "[two](./two.md#test-placement)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a heading with punctuation",
            headings: "## Test placement, done!\n",
            links: "[two](./two.md#test-placement-done)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a heading underlined with equals signs",
            headings: "Test placement\n==============\n",
            links: "[two](./two.md#test-placement)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a heading that spans two lines",
            headings: "Two line\nheading\n===\n",
            links: "[two](./two.md#two-lineheading)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a two-line heading linked with a hyphen at the break",
            headings: "Two line\nheading\n===\n",
            links: "[two](./two.md#two-line-heading)\n",
            first_missing: Some("two-line-heading"),
        },
        AnchorCase {
            name: "a heading underlined with dashes",
            headings: "Second part\n-----------\n",
            links: "[two](./two.md#second-part)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "three duplicate headings linked together",
            headings: "# Test placement\n## Test placement\n### Test placement\n",
            links: "[a](./two.md#test-placement) [b](./two.md#test-placement-1) [c](./two.md#test-placement-2)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a number past the duplicates after valid links",
            headings: "# Test placement\n## Test placement\n",
            links: "[a](./two.md#test-placement) [b](./two.md#test-placement-1) [c](./two.md#test-placement-2)\n",
            first_missing: Some("test-placement-2"),
        },
        AnchorCase {
            name: "the setup suffixes linked together",
            headings: "# Setup\n# Setup-1\n# Setup\n",
            links: "[a](./two.md#setup) [b](./two.md#setup-1) [c](./two.md#setup-2)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "both setext heading kinds in one file",
            headings: "Test placement\n==============\n\nSecond part\n-----------\n",
            links: "[a](./two.md#test-placement) [b](./two.md#second-part)\n",
            first_missing: None,
        },
        AnchorCase {
            name: "a visible heading after a hidden one",
            headings: "```\n## Hidden\n```\n## Shown\n",
            links: "[a](./two.md#shown) [b](./two.md#hidden)\n",
            first_missing: Some("hidden"),
        },
        AnchorCase {
            name: "a heading inside an HTML comment",
            headings: "<!--\n## Hidden\n-->\n\n## Shown\n",
            links: "[two](./two.md#hidden)\n",
            first_missing: Some("hidden"),
        },
    ];

    for case in cases {
        let files = [
            file("docs/stomme/shared/one.md", case.links),
            file("docs/stomme/shared/two.md", case.headings),
        ];
        let expected = match case.first_missing {
            None => Ok(()),
            Some(anchor) => Err(LinkError::MissingAnchor {
                path: "docs/stomme/shared/one.md".to_string(),
                target: format!("./two.md#{anchor}"),
                anchor: anchor.to_string(),
                resolved: "docs/stomme/shared/two.md".to_string(),
            }),
        };

        assert_eq!(validate_links(&files), expected, "{}", case.name);
    }
}

#[test]
fn validate_links_checks_a_reference_style_link_with_a_definition() {
    let cases = [
        (
            "a defined link to a file in the set",
            "[two][ref]\n\n[ref]: ./two.md\n",
            Ok(()),
        ),
        (
            "a defined link to a heading in the set",
            "[two][ref]\n\n[ref]: ./two.md#part\n",
            Ok(()),
        ),
        (
            "a defined link to a missing file",
            "[two][ref]\n\n[ref]: ./missing.md\n",
            Err(LinkError::Broken {
                path: "docs/stomme/shared/one.md".to_string(),
                target: "./missing.md".to_string(),
            }),
        ),
        (
            "a defined link to a missing anchor",
            "[two][ref]\n\n[ref]: ./two.md#missing\n",
            Err(LinkError::MissingAnchor {
                path: "docs/stomme/shared/one.md".to_string(),
                target: "./two.md#missing".to_string(),
                anchor: "missing".to_string(),
                resolved: "docs/stomme/shared/two.md".to_string(),
            }),
        ),
    ];

    for (name, content, expected) in cases {
        let files = [
            file("docs/stomme/shared/one.md", content),
            file("docs/stomme/shared/two.md", "# Two\n\n## Part\n"),
        ];

        assert_eq!(validate_links(&files), expected, "{name}");
    }
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
fn a_fence_line_with_trailing_text_does_not_close_a_fence_for_links() {
    let text = "```\n```not a closer\n[inside](./inside.md)\n```\n";

    assert_eq!(link_targets(text), Vec::<String>::new());
}

#[test]
fn a_balanced_paren_destination_resolves_in_full() {
    let files = [
        file("docs/stomme/shared/one.md", "[two](./two_(draft).md)\n"),
        file("docs/stomme/shared/two_(draft).md", "# Two\n"),
    ];

    assert_eq!(
        link_targets("[two](./two_(draft).md)\n"),
        vec!["./two_(draft).md"]
    );
    assert_eq!(validate_links(&files), Ok(()));
}

#[test]
fn validate_links_drops_a_link_title() {
    let files = [
        file("docs/stomme/shared/one.md", "[two](./two.md \"Two\")\n"),
        file("docs/stomme/shared/two.md", "# Two\n"),
    ];

    assert_eq!(validate_links(&files), Ok(()));
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

fn smallest_selection_links(catalog: &Catalog, name: &str) -> Result<(), LinkError> {
    let selection = smallest_selection(catalog, name)
        .expect("the smallest selection passes the real selection rules");

    validate_links(&owned_set(&selection, catalog))
}

#[test]
fn every_layer_passes_links_in_its_smallest_selection() {
    let catalog = crate::layer::catalog();

    for (name, _) in catalog.guides() {
        assert_eq!(
            smallest_selection_links(catalog, name),
            Ok(()),
            "layer `{name}` fails links in its smallest selection"
        );
    }
}

#[test]
fn a_layer_guide_linking_outside_its_smallest_selection_fails() {
    let catalog = crate::layer::test_guide_catalog(&[
        ("one", "language = \"rust\"\n", "[other](../two/guide.md)\n"),
        ("two", "language = \"rust\"\n", "# Two\n"),
    ]);

    assert_eq!(
        smallest_selection_links(&catalog, "one"),
        Err(LinkError::Broken {
            path: "docs/stomme/layers/one/guide.md".to_string(),
            target: "../two/guide.md".to_string(),
        })
    );
    assert_eq!(smallest_selection_links(&catalog, "two"), Ok(()));
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

#[test]
fn a_smallest_selection_the_selection_rules_reject_is_an_error() {
    let catalog = crate::layer::test_guide_catalog(&[
        (
            "one",
            "language = \"rust\"\nrequires = [\"two\"]\n",
            "# One\n",
        ),
        (
            "two",
            "language = \"rust\"\nconflicts = [\"one\"]\n",
            "# Two\n",
        ),
    ]);

    assert_eq!(
        smallest_selection(&catalog, "one").map(|_| ()),
        Err(SelectionError::Conflict {
            layer: "two".to_string(),
            conflict: "one".to_string(),
        })
    );
}

fn smallest_selection(catalog: &Catalog, name: &str) -> Result<Selection, SelectionError> {
    let (language, layers) = catalog
        .smallest_selection(name)
        .expect("a registered layer has a language and requirements");
    let layers = crate::layer::select(catalog, language, layers.into_iter().collect())?;

    Ok(Selection {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language,
        layers,
        roles: BTreeMap::new(),
        violations: Vec::new(),
        extra_docs: Vec::new(),
        modules: BTreeMap::new(),
    })
}
