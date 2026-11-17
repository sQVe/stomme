use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::*;
use crate::layer;

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();

    collect(root, root, &mut files);

    files
}

fn collect(root: &Path, folder: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(folder).expect("read a folder") {
        let entry = entry.expect("read a folder entry");
        let path = entry.path();

        if entry.file_type().expect("read the file type").is_dir() {
            collect(root, &path, files);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("a file lies inside the root")
                .to_string_lossy()
                .replace('\\', "/");

            files.insert(relative, fs::read(&path).expect("read a file"));
        }
    }
}

fn record(root: &Path) -> Record {
    let text = fs::read_to_string(root.join(BUNDLE_RECORD)).expect("read the bundle record");

    toml::from_str(&text).expect("parse the bundle record")
}

#[test]
fn guides_writes_every_file_and_a_matching_record() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let expected = owned::bundle_set(layer::catalog());

    run(root.path(), layer::catalog()).expect("write the bundle");

    for file in &expected {
        let key = sync::record_key(&file.path).expect("a bundle path is under docs/stomme");
        let written = fs::read(root.path().join(key)).expect("read a bundled file");

        assert_eq!(written, file.content.as_bytes(), "{key}");
    }

    let record = record(root.path());

    assert_eq!(record.schema, RECORD_SCHEMA);
    assert_eq!(record.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(record.agents, None);
    assert_eq!(record.files.len(), expected.len());

    for file in &expected {
        let key = sync::record_key(&file.path).expect("a bundle path is under docs/stomme");

        assert_eq!(
            record.files.get(key),
            Some(&sync::hash(file.content.as_bytes())),
            "{key}"
        );
    }
}

#[test]
fn guides_twice_changes_nothing() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    run(root.path(), layer::catalog()).expect("write the bundle");
    let first = snapshot(root.path());

    run(root.path(), layer::catalog()).expect("rewrite the bundle");

    assert_eq!(snapshot(root.path()), first);
}

#[test]
fn guides_refuses_an_edited_file() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    run(root.path(), layer::catalog()).expect("write the bundle");

    let path = root.path().join("shared/coding.md");

    fs::write(&path, "local\n").expect("edit the bundled file");

    let error = run(root.path(), layer::catalog()).expect_err("the run refuses");

    assert!(
        matches!(error, GuidesError::Edited { ref path } if path == "shared/coding.md"),
        "{error:?}"
    );
    assert_eq!(fs::read_to_string(&path).expect("read the file"), "local\n");
}

#[test]
fn guides_refuses_an_unrecorded_file() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::create_dir(root.path().join("shared")).expect("create the shared folder");
    fs::write(root.path().join("shared/coding.md"), "local\n").expect("write the local file");

    let error = run(root.path(), layer::catalog()).expect_err("the run refuses");

    assert!(
        matches!(error, GuidesError::Unrecorded { ref path } if path == "shared/coding.md"),
        "{error:?}"
    );
    assert!(!root.path().join(BUNDLE_RECORD).exists());
}

#[test]
fn guides_refuses_a_malformed_record() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::write(root.path().join(BUNDLE_RECORD), "not toml\n").expect("write the record");

    let error = run(root.path(), layer::catalog()).expect_err("the run refuses");

    assert!(
        matches!(
            error,
            GuidesError::Sync(SyncError::MalformedRecord { ref path, .. }) if path == BUNDLE_RECORD
        ),
        "{error:?}"
    );
    assert!(!root.path().join("shared/coding.md").exists());
}

#[test]
fn guides_refuses_another_schema() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::write(
        root.path().join(BUNDLE_RECORD),
        "schema = 2\nversion = \"1.0.0\"\n",
    )
    .expect("write the record");

    let error = run(root.path(), layer::catalog()).expect_err("the run refuses");
    let message = error.to_string();

    assert!(message.contains("schema 2"), "{message}");
    assert!(message.contains("schema 1"), "{message}");
}

#[test]
fn guides_removes_a_recorded_file_the_release_dropped() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    run(root.path(), layer::catalog()).expect("write the bundle");

    let mut saved = record(root.path());
    let path = root.path().join("old.md");

    fs::write(&path, "old\n").expect("write the old file");
    saved
        .files
        .insert("old.md".to_string(), sync::hash(b"old\n"));

    let text = toml::to_string(&saved).expect("serialize the record");

    fs::write(root.path().join(BUNDLE_RECORD), text).expect("write the record");
    run(root.path(), layer::catalog()).expect("rewrite the bundle");

    assert!(!path.exists());
    assert!(!record(root.path()).files.contains_key("old.md"));
}

#[test]
fn guides_leaves_an_unrelated_file_alone() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::write(root.path().join("notes.md"), "notes\n").expect("write an unrelated file");
    run(root.path(), layer::catalog()).expect("write the bundle");

    assert_eq!(
        fs::read_to_string(root.path().join("notes.md")).expect("read the file"),
        "notes\n"
    );
}

#[test]
fn guides_refuses_a_file_as_the_directory() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let path = root.path().join("bundle");

    fs::write(&path, "file\n").expect("write a file");

    let error = run(&path, layer::catalog()).expect_err("the run refuses");

    assert!(
        matches!(error, GuidesError::NotADirectory { .. }),
        "{error:?}"
    );
}

#[cfg(unix)]
#[test]
fn guides_refuses_a_symlinked_parent() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("create a temporary directory");
    let outside = tempfile::tempdir().expect("create an outside directory");

    symlink(outside.path(), root.path().join("shared")).expect("create the symlink");

    let error = run(root.path(), layer::catalog()).expect_err("the run refuses");

    assert!(
        matches!(
            error,
            GuidesError::Sync(SyncError::SymlinkedFolder { ref path }) if path == "shared"
        ),
        "{error:?}"
    );
    assert_eq!(snapshot(outside.path()), BTreeMap::new());
}
