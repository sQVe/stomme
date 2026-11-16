use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::config::Selection;
use crate::layer::Language;
use crate::owned::OwnedFile;

fn project(config: &str) -> TempDir {
    let root = tempfile::Builder::new()
        .prefix("stomme-sync-")
        .tempdir()
        .expect("create a temporary directory");

    fs::write(root.path().join("stomme.toml"), config).expect("write stomme.toml");

    root
}

fn rust_project(layers: &[&str]) -> TempDir {
    let layers = layers
        .iter()
        .map(|layer| format!("\"{layer}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let config = format!(
        "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\nlayers = [{layers}]\n",
        env!("CARGO_PKG_VERSION")
    );

    project(&config)
}

fn typescript_project(layers: &[&str]) -> TempDir {
    let layers = layers
        .iter()
        .map(|layer| format!("\"{layer}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let config = format!(
        "schema = 1\nversion = \"{}\"\nlanguage = \"typescript\"\nlayers = [{layers}]\n",
        env!("CARGO_PKG_VERSION")
    );

    project(&config)
}

fn sync_project(root: &Path) -> Result<(), SyncError> {
    run(root, crate::layer::catalog())
}

fn write_record(root: &Path, path: &str) {
    let record = format!(
        "version = \"{}\"\n[files]\n\"{path}\" = \"hash\"\n",
        env!("CARGO_PKG_VERSION")
    );

    fs::create_dir_all(root.join("docs/stomme")).expect("create docs/stomme");
    fs::write(root.join(RECORD_FILE), record).expect("write the record");
}

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();

    collect_files(root, root, &mut files);

    files
}

fn collect_files(root: &Path, folder: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(folder).expect("read a folder") {
        let entry = entry.expect("read a folder entry");
        let path = entry.path();

        if entry.file_type().expect("read the file type").is_dir() {
            collect_files(root, &path, files);
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

fn selection(language: Language) -> Selection {
    Selection {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language,
        layers: BTreeSet::new(),
        roles: BTreeMap::new(),
        extra_docs: Vec::new(),
    }
}

#[test]
fn sync_twice_changes_nothing() {
    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");
    let first = snapshot(root.path());

    sync_project(root.path()).expect("the second sync succeeds");

    assert_eq!(snapshot(root.path()), first);
}

#[test]
fn sync_writes_the_guides_for_the_selected_layers() {
    let root = rust_project(&["gpui", "gpui-component"]);

    sync_project(root.path()).expect("sync succeeds");

    assert!(
        root.path()
            .join("docs/stomme/layers/gpui/guide.md")
            .is_file()
    );
    assert!(
        root.path()
            .join("docs/stomme/layers/gpui-component/guide.md")
            .is_file()
    );
    assert!(
        !root
            .path()
            .join("docs/stomme/layers/herdr/guide.md")
            .exists()
    );
}

#[test]
fn sync_does_not_write_a_gpui_guide_for_herdr() {
    let root = typescript_project(&["herdr"]);

    sync_project(root.path()).expect("sync succeeds");

    assert!(
        root.path()
            .join("docs/stomme/layers/herdr/guide.md")
            .is_file()
    );
    assert!(
        !root
            .path()
            .join("docs/stomme/layers/gpui/guide.md")
            .exists()
    );
}

#[test]
fn sync_removes_the_guide_of_a_dropped_layer() {
    let root = rust_project(&["gpui"]);

    sync_project(root.path()).expect("the first sync succeeds");

    let guide = root.path().join("docs/stomme/layers/gpui/guide.md");

    assert!(guide.is_file());

    fs::write(
        root.path().join("stomme.toml"),
        format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n",
            env!("CARGO_PKG_VERSION")
        ),
    )
    .expect("write stomme.toml");

    sync_project(root.path()).expect("the second sync succeeds");

    assert!(!guide.exists());
}

#[test]
fn sync_refuses_to_remove_a_locally_edited_guide() {
    let root = rust_project(&["gpui"]);

    sync_project(root.path()).expect("the first sync succeeds");

    let guide = root.path().join("docs/stomme/layers/gpui/guide.md");

    fs::write(&guide, "local edit\n").expect("edit the guide");
    fs::write(
        root.path().join("stomme.toml"),
        format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n",
            env!("CARGO_PKG_VERSION")
        ),
    )
    .expect("write stomme.toml");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Edited { ref path } if path == "docs/stomme/layers/gpui/guide.md"),
        "{error:?}"
    );
    assert_eq!(
        fs::read_to_string(&guide).expect("read the guide"),
        "local edit\n"
    );
}

#[test]
fn sync_writes_nothing_when_the_generated_set_has_a_broken_link() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let owned = [OwnedFile {
        path: "docs/stomme/shared/one.md".to_string(),
        content: "[missing](./two.md)\n",
    }];

    let error = sync(root.path(), &selection(Language::Rust), &owned).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Link(LinkError::Broken { .. })),
        "{error:?}"
    );
    assert_eq!(snapshot(root.path()), BTreeMap::new());
}

#[test]
fn sync_refuses_a_malformed_record() {
    let root = rust_project(&[]);

    fs::create_dir_all(root.path().join("docs/stomme")).expect("create docs/stomme");
    fs::write(root.path().join(RECORD_FILE), "not toml\n").expect("write the record");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::MalformedRecord { ref path, .. } if path == RECORD_FILE),
        "{error:?}"
    );
    assert!(!root.path().join("docs/stomme/shared/coding.md").exists());
}

#[test]
fn sync_refuses_an_extra_file_under_docs_stomme() {
    let root = rust_project(&[]);

    fs::create_dir_all(root.path().join("docs/stomme")).expect("create docs/stomme");
    fs::write(root.path().join("docs/stomme/extra.md"), "extra\n").expect("write the extra file");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Extra { ref path } if path == "docs/stomme/extra.md"),
        "{error:?}"
    );
    assert!(!root.path().join("docs/stomme/shared/coding.md").exists());
}

#[test]
fn sync_refuses_a_directory_where_a_file_belongs() {
    let root = rust_project(&[]);

    fs::create_dir_all(root.path().join("docs/stomme/shared/coding.md"))
        .expect("create a directory in place of a file");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::NotAFile { ref path } if path == "docs/stomme/shared/coding.md"),
        "{error:?}"
    );
}

#[cfg(unix)]
#[test]
fn sync_refuses_a_non_utf8_entry_name() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let root = rust_project(&[]);
    let folder = root
        .path()
        .join("docs/stomme")
        .join(OsStr::from_bytes(b"\xff"));
    let extra = folder.join("extra.md");

    fs::create_dir_all(&folder).expect("create a folder with a non-UTF-8 name");
    fs::write(&extra, "extra\n").expect("write the extra file");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(error.to_string().contains("\\xFF"), "{error}");
    assert!(error.to_string().contains("UTF-8"), "{error}");
    assert!(extra.exists());
    assert!(!root.path().join(RECORD_FILE).exists());
}

#[cfg(unix)]
#[test]
fn sync_refuses_a_symlinked_docs_folder() {
    use std::os::unix::fs::symlink;

    let root = rust_project(&[]);
    let outside = tempfile::tempdir().expect("create an outside directory");

    symlink(outside.path(), root.path().join("docs")).expect("create the docs symlink");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::SymlinkedFolder { ref path } if path == "docs"),
        "{error:?}"
    );
    assert_eq!(snapshot(outside.path()), BTreeMap::new());
}

#[cfg(unix)]
#[test]
fn sync_refuses_a_symlinked_docs_stomme_folder() {
    use std::os::unix::fs::symlink;

    let root = rust_project(&[]);
    let outside = tempfile::tempdir().expect("create an outside directory");

    fs::create_dir(root.path().join("docs")).expect("create docs");
    symlink(outside.path(), root.path().join("docs/stomme"))
        .expect("create the docs/stomme symlink");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::SymlinkedFolder { ref path } if path == "docs/stomme"),
        "{error:?}"
    );
    assert_eq!(snapshot(outside.path()), BTreeMap::new());
}

#[cfg(unix)]
#[test]
fn sync_refuses_an_extra_file_with_a_literal_backslash() {
    let root = rust_project(&[]);

    fs::create_dir_all(root.path().join("docs/stomme")).expect("create docs/stomme");
    fs::write(root.path().join("docs/stomme/shared\\coding.md"), "extra\n")
        .expect("write the extra file");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Extra { ref path } if path == "docs/stomme/shared\\coding.md"),
        "{error:?}"
    );
    assert!(!root.path().join("docs/stomme/shared/coding.md").exists());
}

#[cfg(unix)]
#[test]
fn sync_refuses_a_symlink_where_a_file_belongs() {
    use std::os::unix::fs::symlink;

    let root = rust_project(&[]);
    let target = root.path().join("target.md");

    fs::write(&target, "target\n").expect("write the target");
    fs::create_dir_all(root.path().join("docs/stomme/shared")).expect("create docs/stomme/shared");
    symlink(&target, root.path().join("docs/stomme/shared/coding.md")).expect("create the symlink");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::NotAFile { ref path } if path == "docs/stomme/shared/coding.md"),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_an_existing_file_that_differs_on_the_first_sync() {
    let root = rust_project(&[]);

    fs::create_dir_all(root.path().join("docs/stomme/shared")).expect("create docs/stomme/shared");
    fs::write(root.path().join("docs/stomme/shared/coding.md"), "local\n")
        .expect("write the local file");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Edited { ref path } if path == "docs/stomme/shared/coding.md"),
        "{error:?}"
    );
    assert!(!root.path().join("docs/stomme/shared/rules.md").exists());
}

#[test]
fn sync_writes_a_missing_owned_file() {
    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");

    let path = root.path().join("docs/stomme/shared/coding.md");

    fs::remove_file(&path).expect("remove the owned file");
    sync_project(root.path()).expect("the second sync succeeds");

    assert!(path.is_file());
}

#[test]
fn sync_refuses_a_locally_edited_owned_file() {
    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");

    let path = root.path().join("docs/stomme/shared/coding.md");

    fs::write(&path, "local edit\n").expect("edit the owned file");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Edited { ref path } if path == "docs/stomme/shared/coding.md"),
        "{error:?}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("read the owned file"),
        "local edit\n"
    );
}

#[test]
fn sync_refuses_a_record_that_names_the_record_file() {
    let root = rust_project(&[]);
    write_record(root.path(), RECORD_FILE);

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(
            error,
            SyncError::MalformedRecord { ref reason, .. }
                if reason == "names `docs/stomme/.sync-record.toml`, which is not an owned file"
        ),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_a_record_that_names_a_path_outside_docs_stomme() {
    let root = rust_project(&[]);
    write_record(root.path(), "README.md");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(
            error,
            SyncError::MalformedRecord { ref reason, .. }
                if reason == "names `README.md`, which is not an owned file"
        ),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_a_record_path_with_an_empty_part() {
    let root = rust_project(&[]);
    write_record(root.path(), "docs/stomme//coding.md");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(
            error,
            SyncError::MalformedRecord { ref reason, .. }
                if reason == "names `docs/stomme//coding.md`, which is not an owned file"
        ),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_a_record_path_with_a_current_folder_part() {
    let root = rust_project(&[]);
    write_record(root.path(), "docs/stomme/./coding.md");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(
            error,
            SyncError::MalformedRecord { ref reason, .. }
                if reason == "names `docs/stomme/./coding.md`, which is not an owned file"
        ),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_a_record_path_with_a_parent_folder_part() {
    let root = rust_project(&[]);
    write_record(root.path(), "docs/stomme/../coding.md");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(
            error,
            SyncError::MalformedRecord { ref reason, .. }
                if reason == "names `docs/stomme/../coding.md`, which is not an owned file"
        ),
        "{error:?}"
    );
}
