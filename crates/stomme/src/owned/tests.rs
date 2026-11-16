use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_files(root: &Path, folder: &Path, files: &mut BTreeSet<String>) {
    for entry in fs::read_dir(root.join(folder)).expect("read a source folder") {
        let entry = entry.expect("read a source entry");
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("a source file lies inside the repository")
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
    let root = repository_root();
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
        file("docs/stomme/shared/two.md", "# Two\n"),
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
fn validate_links_ignores_external_and_anchor_links() {
    let files = [file(
        "docs/stomme/shared/one.md",
        "[a](https://example.com) [b](#part) [c](/absolute.md)\n",
    )];

    assert_eq!(validate_links(&files), Ok(()));
}
