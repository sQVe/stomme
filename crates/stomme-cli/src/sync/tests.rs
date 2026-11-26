use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

use super::*;
use crate::config::Selection;
use crate::layer::Language;
use crate::ocr_rules::{OcrRulesError, PROJECT_RULE_FILE};
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
    run(
        root,
        crate::layer::catalog(),
        &crate::config::FileRead::filesystem(),
    )
}

fn write_record(root: &Path, path: &str) {
    let record = format!(
        "version = \"{}\"\n[files]\n\"{path}\" = \"hash\"\n",
        env!("CARGO_PKG_VERSION")
    );

    fs::create_dir_all(root.join("docs/stomme")).expect("create docs/stomme");
    fs::write(root.join(RECORD_FILE), record).expect("write the record");
}

const PREVIOUS_BLOCK: &str = "<!-- stomme:start -->\n# Shared rules\n\n- Read [the Rust guide](docs/stomme/languages/rust/guide.md).\n<!-- stomme:end -->\n";
const PREVIOUS_GUIDE: &str = "old guide\n";
const CURRENT_GUIDE: &str = "current guide\n";

fn install_record_fixture(root: &Path, name: &str) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sync-records")
        .join(name);
    let destination = root.join(RECORD_FILE);

    fs::create_dir_all(destination.parent().expect("the record has a parent"))
        .expect("create docs/stomme");
    fs::copy(source, destination).expect("copy the record fixture");
}

fn bundled_content(path: &str) -> String {
    crate::owned::bundle_set(crate::layer::catalog())
        .into_iter()
        .find(|file| file.path == path)
        .expect("the bundle holds the path")
        .content
}

fn guide_block(guide: &str) -> String {
    let content = crate::ocr_rules::without_marked_blocks(&bundled_content(guide));

    format!("File: {guide}\n\n{}", content.trim_end())
}

fn expected_guide_text(guides: &[&str]) -> String {
    let intro = "Apply stomme's coding guides below. Each guide starts with its repository path. Resolve its relative links from that path, and read linked files with file_read.";
    let outro = "Also apply the rules in the project's AGENTS.md and the decisions in docs/adr/ that cover the change. Read them with file_read.";
    let blocks = guides
        .iter()
        .map(|guide| guide_block(guide))
        .collect::<Vec<_>>()
        .join("\n\n");

    format!("{intro}\n\n{blocks}\n\n{outro}")
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
        violations: Vec::new(),
        extra_docs: Vec::new(),
        modules: BTreeMap::new(),
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
fn sync_twice_writes_no_file() {
    use std::fs::FileTimes;
    use std::time::{Duration, SystemTime};

    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");

    let record = root.path().join(RECORD_FILE);
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);

    fs::OpenOptions::new()
        .write(true)
        .open(&record)
        .expect("open the record")
        .set_times(FileTimes::new().set_modified(modified))
        .expect("set the modification time");

    sync_project(root.path()).expect("the second sync succeeds");

    let actual = fs::metadata(&record)
        .expect("read the metadata")
        .modified()
        .expect("read the modification time");

    assert_eq!(actual, modified);
}

#[test]
fn sync_writes_the_ocr_rule_file() {
    let root = rust_project(&["gpui"]);

    sync_project(root.path()).expect("sync succeeds");

    let path = "docs/stomme/ocr-rules.json";
    let text = fs::read_to_string(root.path().join(path)).expect("read the rule file");
    let document: serde_json::Value = serde_json::from_str(&text).expect("the rule file is JSON");
    let rules = document["rules"].as_array().expect("the rules are a list");

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["path"], "**/*.rs");
    assert_eq!(rules[0]["merge_system_rule"], serde_json::Value::Bool(true));

    let rule = rules[0]["rule"].as_str().expect("the rule is text");
    let guides = [
        "docs/stomme/shared/coding.md",
        "docs/stomme/languages/rust/guide.md",
        "docs/stomme/layers/gpui/guide.md",
    ];

    assert_eq!(rule, &expected_guide_text(&guides));

    let record = Record::read(root.path())
        .expect("read the record")
        .expect("the record exists");
    let key = crate::owned::record_key(path).expect("a project path");

    assert_eq!(record.files.get(key), Some(&hash(text.as_bytes())));
}

#[test]
fn sync_merges_project_ocr_rules() {
    let root = rust_project(&[]);

    fs::create_dir_all(root.path().join(".opencodereview")).expect("create the rule folder");
    fs::write(
        root.path().join(PROJECT_RULE_FILE),
        r#"{
            "rules": [
                { "path": "src/**", "rule": "Keep modules small." },
                { "path": "tests/**", "rule": "Name the behavior." }
            ],
            "exclude": ["fixtures/**"]
        }"#,
    )
    .expect("write the rule file");

    sync_project(root.path()).expect("sync succeeds");

    let text = fs::read_to_string(root.path().join("docs/stomme/ocr-rules.json"))
        .expect("read the rule file");
    let document: serde_json::Value = serde_json::from_str(&text).expect("the rule file is JSON");
    let rules = document["rules"].as_array().expect("the rules are a list");

    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0]["path"], "src/**");
    assert_eq!(rules[1]["path"], "tests/**");
    assert_eq!(rules[2]["path"], "**/*.rs");

    let guides = [
        "docs/stomme/shared/coding.md",
        "docs/stomme/languages/rust/guide.md",
    ];
    let guide_text = expected_guide_text(&guides);
    let system_rule = rules[2]["rule"].as_str().expect("the rule is text");

    assert_eq!(system_rule, &guide_text);

    for (index, marker) in ["Keep modules small.", "Name the behavior."]
        .into_iter()
        .enumerate()
    {
        let rule = rules[index]["rule"].as_str().expect("the rule is text");
        let expected = format!("{marker}\n\n{guide_text}");

        assert_eq!(rule, &expected, "{marker}");
    }

    assert_eq!(document["exclude"], serde_json::json!(["fixtures/**"]));
}

#[test]
fn sync_refuses_an_invalid_project_rule_file() {
    type DataCheck = fn(&OcrRulesError) -> bool;
    let cases: [(&str, &str, DataCheck); 5] = [
        ("invalid JSON", "{ not json", |error| {
            matches!(error, OcrRulesError::InvalidJson { path, reason }
                if path == PROJECT_RULE_FILE && reason.contains("line 1"))
        }),
        ("an unknown key", r#"{"unknown": true}"#, |error| {
            matches!(error, OcrRulesError::Shape { path, reason }
                if path == PROJECT_RULE_FILE && reason.contains("unknown field `unknown`"))
        }),
        (
            "an empty path",
            r#"{"rules":[{"path":"","rule":"x"}]}"#,
            |error| {
                matches!(error, OcrRulesError::EmptyPath { path, index }
                    if path == PROJECT_RULE_FILE && *index == 1)
            },
        ),
        (
            "a null language",
            r#"{"rules":[{"path":"packages/**","rule":"x","language":null}]}"#,
            |error| {
                matches!(error, OcrRulesError::LanguageNotString { path, index, value }
                    if path == PROJECT_RULE_FILE && *index == 1 && value == "null")
            },
        ),
        (
            "an unknown language",
            r#"{"rules":[{"path":"packages/**","rule":"x","language":"go"}]}"#,
            |error| {
                matches!(error, OcrRulesError::UnknownLanguage { path, index, language }
                    if path == PROJECT_RULE_FILE && *index == 1 && language == "go")
            },
        ),
    ];

    for (name, text, check_data) in cases {
        let root = rust_project(&[]);

        fs::create_dir_all(root.path().join(".opencodereview")).expect("create the rule folder");
        fs::write(root.path().join(PROJECT_RULE_FILE), text).expect("write the rule file");

        let before = snapshot(root.path());
        let error = sync_project(root.path()).expect_err("sync refuses");

        match &error {
            SyncError::OcrRules(inner) => assert!(check_data(inner), "{name}: {error:?}"),
            other => panic!("{name}: expected an OcrRules error, got {other:?}"),
        }

        assert!(
            !root.path().join(crate::owned::DOCS_FOLDER).exists(),
            "{name}: sync created the output folder"
        );
        assert_eq!(snapshot(root.path()), before, "{name}");
    }
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
        content: "[missing](./two.md)\n".to_string(),
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

    fs::write(
        outside.path().join(format!("{STAGING_PREFIX}1.0.notes")),
        "outside\n",
    )
    .expect("write the outside file");
    let before = snapshot(outside.path());

    fs::create_dir(root.path().join("docs")).expect("create docs");
    symlink(outside.path(), root.path().join("docs/stomme"))
        .expect("create the docs/stomme symlink");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::SymlinkedFolder { ref path } if path == "docs/stomme"),
        "{error:?}"
    );
    assert_eq!(snapshot(outside.path()), before);
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
fn sync_does_not_write_through_a_hard_link() {
    let root = rust_project(&[]);
    let outside = tempfile::tempdir().expect("create a temporary directory");
    let path = "docs/stomme/ocr-rules.json";

    sync_project(root.path()).expect("the first sync succeeds");

    let synced = root.path().join(path);
    let alias = outside.path().join("ocr-rules.json");
    let before = fs::read(&synced).expect("read the synced file");

    fs::hard_link(&synced, &alias).expect("link the synced file");
    add_project_rule(root.path());

    sync_project(root.path()).expect("the second sync succeeds");

    assert_eq!(fs::read(&alias).expect("read the linked file"), before);
    assert_ne!(fs::read(&synced).expect("read the synced file"), before);
}

#[cfg(unix)]
#[test]
fn sync_keeps_the_permissions_of_a_synced_file() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = rust_project(&[]);
    let path = "docs/stomme/ocr-rules.json";

    sync_project(root.path()).expect("the first sync succeeds");

    let synced = root.path().join(path);
    let before = fs::read(&synced).expect("read the synced file");

    fs::set_permissions(&synced, fs::Permissions::from_mode(0o640))
        .expect("set the permissions of the synced file");
    add_project_rule(root.path());

    sync_project(root.path()).expect("the second sync succeeds");

    assert_ne!(
        fs::read(&synced).expect("read the synced file"),
        before,
        "the second sync rewrites the file"
    );

    let metadata = fs::metadata(&synced).expect("read the metadata of the synced file");

    assert_eq!(metadata.permissions().mode() & 0o777, 0o640);
}

#[cfg(unix)]
#[test]
fn sync_refuses_a_read_only_owned_file() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = rust_project(&[]);
    let path = "docs/stomme/ocr-rules.json";

    sync_project(root.path()).expect("the first sync succeeds");

    let synced = root.path().join(path);
    let before = fs::read(&synced).expect("read the synced file");

    fs::set_permissions(&synced, fs::Permissions::from_mode(0o400))
        .expect("set the read-only mode");
    add_project_rule(root.path());

    if fs::OpenOptions::new().write(true).open(&synced).is_ok() {
        // A user that ignores file modes, such as root, cannot show the refusal.
        return;
    }

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Write { ref path, .. } if path == "docs/stomme/ocr-rules.json"),
        "{error:?}"
    );
    assert_eq!(fs::read(&synced).expect("read the synced file"), before);
}

#[test]
fn sync_removes_a_stale_staging_file() {
    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");

    let stale = root
        .path()
        .join("docs/stomme/shared")
        .join(format!("{STAGING_PREFIX}4242.0.coding.md"));

    fs::write(&stale, "partial\n").expect("write the stale staging file");

    sync_project(root.path()).expect("the second sync succeeds");

    assert!(!stale.exists());
}

#[test]
fn sync_removes_a_stale_staging_file_beside_the_agents_block() {
    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");

    let stale = root
        .path()
        .join(format!("{STAGING_PREFIX}4242.0.AGENTS.md"));

    fs::write(&stale, "partial\n").expect("write the stale staging file");

    sync_project(root.path()).expect("the second sync succeeds");

    assert!(!stale.exists());
}

#[test]
fn sync_keeps_a_stale_staging_file_when_it_refuses() {
    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");

    let stale = root
        .path()
        .join("docs/stomme/shared")
        .join(format!("{STAGING_PREFIX}4242.0.coding.md"));
    let extra = root.path().join("docs/stomme/shared/extra.md");

    fs::write(&stale, "partial\n").expect("write the stale staging file");
    fs::write(&extra, "extra\n").expect("write the extra file");

    let before = snapshot(root.path());
    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Extra { ref path } if path == "docs/stomme/shared/extra.md"),
        "{error:?}"
    );
    assert_eq!(snapshot(root.path()), before);
}

#[test]
fn sync_waits_for_a_run_that_holds_the_lock() {
    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");
    add_project_rule(root.path());

    let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.path().join(LOCK_FILE))
        .expect("open the lock file");

    held.lock().expect("take the lock");

    let staging = root
        .path()
        .join("docs/stomme/shared")
        .join(format!("{STAGING_PREFIX}4242.0.coding.md"));

    fs::write(&staging, "partial\n").expect("write the live staging file");

    let path = root.path().to_path_buf();
    let (started, started_rx) = mpsc::channel();
    let (done, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        started.send(()).expect("send the start");
        done.send(sync_project(&path)).expect("send the result");
    });

    started_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("the second run starts");

    let waited = done_rx.recv_timeout(Duration::from_secs(1));

    assert_eq!(
        waited.expect_err("the second run finished while another run held the lock"),
        mpsc::RecvTimeoutError::Timeout
    );
    assert!(
        staging.exists(),
        "the second run removed a live staging file"
    );

    drop(held);

    let result = done_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("the second run finishes once the lock is free");

    result.expect("the second sync succeeds");
    worker.join().expect("the sync thread joins");

    assert!(
        !staging.exists(),
        "the second run removes the staging file after the wait"
    );
}

#[test]
fn sync_refuses_a_record_naming_the_lock_file() {
    let root = rust_project(&[]);

    write_record(root.path(), LOCK_FILE);

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(
            error,
            SyncError::MalformedRecord { ref path, ref reason }
                if path == RECORD_FILE && reason.contains(".sync-lock")
        ),
        "{error:?}"
    );
    assert!(!root.path().join("docs/stomme/shared").exists());
}

#[cfg(unix)]
#[test]
fn sync_refuses_a_symlinked_lock_file() {
    use std::os::unix::fs::symlink;

    let root = rust_project(&[]);
    let outside = tempfile::tempdir().expect("create an outside directory");
    let target = outside.path().join("target");

    fs::create_dir_all(root.path().join("docs/stomme")).expect("create docs/stomme");
    symlink(&target, root.path().join(LOCK_FILE)).expect("create the lock symlink");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::NotAFile { ref path } if path == LOCK_FILE),
        "{error:?}"
    );
    assert!(!target.exists(), "the run created the link target");
}

#[test]
fn sync_refuses_a_user_file_with_a_staging_prefix() {
    let root = rust_project(&[]);

    sync_project(root.path()).expect("the first sync succeeds");

    let user = root
        .path()
        .join("docs/stomme/shared")
        .join(format!("{STAGING_PREFIX}notes"));

    fs::write(&user, "mine\n").expect("write the user file");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Extra { ref path } if path == "docs/stomme/shared/.stomme-staging.notes"),
        "{error:?}"
    );
    assert!(user.exists());
}

#[test]
fn sync_refuses_a_recorded_file_with_a_staging_name() {
    let root = rust_project(&[]);
    let path = "docs/stomme/.stomme-staging.4242.0.notes";
    let recorded = root.path().join(path);

    write_record(root.path(), path);
    fs::create_dir_all(recorded.parent().expect("the file has a parent"))
        .expect("create docs/stomme");
    fs::write(&recorded, "edited\n").expect("write the recorded file");
    fs::write(root.path().join(LOCK_FILE), "").expect("write the lock file");

    let before = snapshot(root.path());
    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Edited { ref path } if path == "docs/stomme/.stomme-staging.4242.0.notes"),
        "{error:?}"
    );
    assert_eq!(snapshot(root.path()), before);
}

#[test]
fn staging_name_matches_the_staging_grammar() {
    let target = Path::new("docs/stomme/shared/coding.md");
    let name = staging_name(target, 42, 3);

    assert_eq!(name, ".stomme-staging.42.3.coding.md");
    assert!(is_staging_name(&name));
}

#[test]
fn write_file_uses_a_free_staging_name() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let path = "docs/stomme/shared/coding.md";
    let target = root.path().join(path);
    let folder = root.path().join("docs/stomme/shared");
    let taken = folder.join(staging_name(&target, std::process::id(), 0));

    fs::create_dir_all(&folder).expect("create the folder");
    fs::write(&taken, "stale\n").expect("write the taken staging name");

    write_file(root.path(), path, "current\n").expect("the write succeeds");

    assert_eq!(
        fs::read_to_string(&target).expect("read the target"),
        "current\n"
    );
    assert_eq!(
        fs::read_to_string(&taken).expect("read the taken staging name"),
        "stale\n"
    );
    assert_eq!(
        fs::read_dir(&folder).expect("read the folder").count(),
        2,
        "the folder holds the target and the taken name"
    );
}

#[cfg(unix)]
#[test]
fn create_exclusive_applies_the_permissions_before_the_first_write() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = tempfile::tempdir().expect("create a temporary directory");
    let path = "docs/stomme/shared/coding.md";
    let target = root.path().join(path);
    let staging = root.path().join("staging");

    fs::create_dir_all(target.parent().expect("the target has a parent"))
        .expect("create the folder");
    fs::write(&target, "old\n").expect("write the target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("set the target mode");
    let permissions = fs::metadata(&target)
        .expect("read the target metadata")
        .permissions();

    let file = create_exclusive(&staging, Some(&permissions), path)
        .expect("create the staging file")
        .expect("the staging name is free");
    let metadata = fs::metadata(&staging).expect("read the staging metadata");

    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    assert_eq!(metadata.len(), 0);

    drop(file);
}

fn add_project_rule(root: &Path) {
    fs::create_dir_all(root.join(".opencodereview")).expect("create the rule folder");
    fs::write(
        root.join(PROJECT_RULE_FILE),
        r#"{ "rules": [{ "path": "src/**", "rule": "Keep modules small." }] }"#,
    )
    .expect("write the project rule file");
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
                if reason == "names `docs/stomme/.sync-record.toml`, which is not a synced file"
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
                if reason == "names `README.md`, which is not a synced file"
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
                if reason == "names `docs/stomme//coding.md`, which is not a synced file"
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
                if reason == "names `docs/stomme/./coding.md`, which is not a synced file"
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
                if reason == "names `docs/stomme/../coding.md`, which is not a synced file"
        ),
        "{error:?}"
    );
}

#[test]
fn sync_writes_the_current_record_shape() {
    let root = rust_project(&[]);

    fs::write(root.path().join("AGENTS.md"), "# Project\n").expect("write AGENTS.md");
    fs::create_dir_all(root.path().join("docs/adr")).expect("create docs/adr");
    fs::write(root.path().join("docs/adr/README.md"), "# ADR\n").expect("write the ADR index");

    sync_project(root.path()).expect("sync succeeds");

    let record = Record::read(root.path())
        .expect("read the record")
        .expect("the record exists");

    assert_eq!(record.schema, RECORD_SCHEMA);
    assert!(record.agents.is_some());
    assert!(record.files.contains_key("shared/coding.md"));
    assert!(!record.files.contains_key("docs/stomme/shared/coding.md"));
    assert!(!record.files.contains_key("AGENTS.md"));
}

#[test]
fn sync_reads_the_frozen_previous_record() {
    let root = rust_project(&[]);

    install_record_fixture(root.path(), "previous.toml");

    let record = Record::read(root.path())
        .expect("read the record")
        .expect("the record exists");

    assert_eq!(record.schema, RECORD_SCHEMA);
    assert_eq!(record.version, "0.0.0");
    assert_eq!(record.agents, Some(hash(PREVIOUS_BLOCK.as_bytes())));
    assert_eq!(
        record.files.get("shared/coding.md"),
        Some(&hash(PREVIOUS_GUIDE.as_bytes()))
    );
    assert!(!record.files.contains_key("AGENTS.md"));
    assert!(!record.files.contains_key("docs/stomme/shared/coding.md"));
}

#[test]
fn sync_rewrites_the_frozen_previous_record() {
    let root = rust_project(&[]);
    let coding = bundled_content("docs/stomme/shared/coding.md");

    fs::write(
        root.path().join("AGENTS.md"),
        format!("# Project\n\n{PREVIOUS_BLOCK}"),
    )
    .expect("write AGENTS.md");
    fs::create_dir_all(root.path().join("docs/adr")).expect("create docs/adr");
    fs::write(root.path().join("docs/adr/README.md"), "# ADR\n").expect("write the ADR index");
    fs::create_dir_all(root.path().join("docs/stomme/shared")).expect("create docs/stomme/shared");
    fs::write(
        root.path().join("docs/stomme/shared/coding.md"),
        PREVIOUS_GUIDE,
    )
    .expect("write the recorded guide");
    install_record_fixture(root.path(), "previous.toml");

    sync_project(root.path()).expect("sync succeeds");

    let record = Record::read(root.path())
        .expect("read the record")
        .expect("the record exists");
    let block = crate::agents_block::render(&selection(Language::Rust), None);

    assert_eq!(record.schema, RECORD_SCHEMA);
    assert_eq!(record.agents, Some(hash(block.as_bytes())));
    assert_eq!(
        record.files.get("shared/coding.md"),
        Some(&hash(coding.as_bytes()))
    );
    assert!(!record.files.contains_key("AGENTS.md"));
    assert!(!record.files.contains_key("docs/stomme/shared/coding.md"));
    assert_eq!(
        fs::read_to_string(root.path().join("docs/stomme/shared/coding.md"))
            .expect("read the rewritten guide"),
        coding
    );
}

#[test]
fn sync_reads_the_frozen_current_record() {
    let root = rust_project(&[]);

    install_record_fixture(root.path(), "current.toml");

    let record = Record::read(root.path())
        .expect("read the record")
        .expect("the record exists");

    assert_eq!(record.schema, RECORD_SCHEMA);
    assert_eq!(record.version, "0.1.0");
    assert_eq!(record.agents, Some(hash(PREVIOUS_BLOCK.as_bytes())));
    assert_eq!(
        record.files.get("shared/coding.md"),
        Some(&hash(CURRENT_GUIDE.as_bytes()))
    );
}

#[test]
fn sync_refuses_a_directory_as_the_record() {
    let root = rust_project(&[]);

    fs::create_dir_all(root.path().join(RECORD_FILE)).expect("create a directory as the record");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(
            error,
            SyncError::MalformedRecord { ref reason, .. } if reason == "is not a regular file"
        ),
        "{error:?}"
    );
}

#[test]
fn sync_refuses_a_current_record_with_an_unsafe_key() {
    let root = rust_project(&[]);

    fs::create_dir_all(root.path().join("docs/stomme")).expect("create docs/stomme");
    fs::write(
        root.path().join(RECORD_FILE),
        format!(
            "schema = 1\nversion = \"{}\"\n[files]\n\"../coding.md\" = \"{}\"\n",
            env!("CARGO_PKG_VERSION"),
            "0".repeat(64),
        ),
    )
    .expect("write the record");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(
            error,
            SyncError::MalformedRecord { ref reason, .. }
                if reason == "names `../coding.md`, which is not a synced file"
        ),
        "{error:?}"
    );
}
