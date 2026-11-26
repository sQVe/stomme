use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../src/test_support.rs"]
mod test_support;

use test_support::{commit, complete_project, copy_tree, git, with_crate_version, write_file};

fn sync_project(root: &Path) {
    let output = stomme(root, &["sync"]);

    assert_eq!(output.status.code(), Some(0), "sync: {output:?}");
}

fn stomme<A: AsRef<OsStr>>(root: &Path, arguments: &[A]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stomme"))
        .args(arguments)
        .current_dir(root)
        .env_remove("GITHUB_BASE_REF")
        .output()
        .expect("run stomme")
}

fn stderr_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_string)
        .collect()
}

fn assert_single_problem(output: &Output, path: &str, rule: &str, case: &str) {
    let lines = stderr_lines(output);

    assert_eq!(output.status.code(), Some(1), "{case}");
    assert_eq!(lines.len(), 1, "{case}: {lines:?}");
    assert!(
        lines[0].starts_with(&format!("stomme: {path}: ")),
        "{case}: {lines:?}"
    );
    assert!(
        lines[0].ends_with(&format!(" [{rule}]")),
        "{case}: {lines:?}"
    );
}

#[test]
fn check_passes_a_complete_project() {
    let root = complete_project();

    let output = stomme(root.path(), &["check"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_lines(&output), Vec::<String>::new());
}

#[test]
fn check_reports_a_git_failure_when_git_dir_is_set() {
    let root = complete_project();
    let output = Command::new(env!("CARGO_BIN_EXE_stomme"))
        .arg("check")
        .current_dir(root.path())
        .env_remove("GITHUB_BASE_REF")
        .env("GIT_DIR", root.path().join("missing-git-dir"))
        .output()
        .expect("run stomme");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr.contains("`git rev-list --all --count` failed"),
        "{stderr}"
    );
}

#[test]
fn check_reports_a_project_without_stomme_toml() {
    let root = complete_project();

    fs::remove_file(root.path().join("stomme.toml")).expect("remove stomme.toml");

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(&output, "stomme.toml", "no-stomme", "missing stomme.toml");
}

#[test]
fn check_reports_a_broken_stomme_toml() {
    let cases = [
        (
            "syntax error",
            format!(
                "schema = 1\nversion = \"{}\"\nlanguage = \"rust\n",
                env!("CARGO_PKG_VERSION")
            ),
        ),
        (
            "unknown field",
            format!(
                "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\nname = \"x\"\n",
                env!("CARGO_PKG_VERSION")
            ),
        ),
        (
            "missing language",
            format!("schema = 1\nversion = \"{}\"\n", env!("CARGO_PKG_VERSION")),
        ),
        (
            "schema 2",
            format!(
                "schema = 2\nversion = \"{}\"\nlanguage = \"rust\"\n",
                env!("CARGO_PKG_VERSION")
            ),
        ),
    ];

    for (case, text) in cases {
        let root = complete_project();

        write_file(root.path(), "stomme.toml", &text);

        let output = stomme(root.path(), &["check"]);

        assert_single_problem(&output, "stomme.toml", "broken-installation", case);
    }
}

#[test]
fn check_reports_a_bad_layer_set_naming_the_layer() {
    let cases = [
        (
            "wrong language",
            "language = \"typescript\"\nlayers = [\"gpui\"]\n",
            "`gpui`",
        ),
        (
            "missing required layer",
            "language = \"rust\"\nlayers = [\"gpui-component\"]\n",
            "`gpui`",
        ),
        (
            "unknown layer",
            "language = \"rust\"\nlayers = [\"egui\"]\n",
            "`egui`",
        ),
        (
            "duplicate layer",
            "language = \"typescript\"\nlayers = [\"react\", \"react\"]\n",
            "`react`",
        ),
        ("leftover kind", "kind = \"rust-cli\"\n", "`language`"),
    ];

    for (case, text, named) in cases {
        let root = complete_project();

        write_file(
            root.path(),
            "stomme.toml",
            &format!(
                "schema = 1\nversion = \"{}\"\n{text}",
                env!("CARGO_PKG_VERSION")
            ),
        );

        let output = stomme(root.path(), &["check"]);

        assert_single_problem(&output, "stomme.toml", "broken-installation", case);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(named),
            "{case}: {output:?}"
        );
    }
}

#[test]
fn check_reports_an_extra_doc_that_does_not_exist() {
    let root = complete_project();

    write_file(
        root.path(),
        "stomme.toml",
        &format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n\n[exceptions]\nextra_docs = [\"docs/missing.md\"]\n",
            env!("CARGO_PKG_VERSION")
        ),
    );

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(
        &output,
        "stomme.toml",
        "broken-installation",
        "missing extra doc",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("docs/missing.md"),
        "{output:?}"
    );
}

#[test]
fn check_reports_an_unknown_exception_field() {
    let root = complete_project();

    write_file(
        root.path(),
        "stomme.toml",
        &format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n\n[exceptions]\nother = []\n",
            env!("CARGO_PKG_VERSION")
        ),
    );

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(
        &output,
        "stomme.toml",
        "broken-installation",
        "unknown exception field",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("other"),
        "{output:?}"
    );
}

#[cfg(unix)]
#[test]
fn check_reports_a_stomme_toml_that_is_a_link() {
    use std::os::unix::fs::symlink;

    let root = complete_project();

    fs::remove_file(root.path().join("stomme.toml")).expect("remove stomme.toml");
    symlink("missing.toml", root.path().join("stomme.toml")).expect("create the link");

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(
        &output,
        "stomme.toml",
        "broken-installation",
        "symbolic link",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("is a symbolic link; use a regular file"),
        "{output:?}"
    );
}

#[derive(Debug, PartialEq, Eq)]
enum SnapshotEntry {
    Directory,
    Link(PathBuf),
    File(Vec<u8>),
}

fn snapshot(root: &Path) -> BTreeMap<String, SnapshotEntry> {
    let mut entries = BTreeMap::new();

    collect_snapshot(root, root, &mut entries);

    entries
}

fn collect_snapshot(root: &Path, folder: &Path, entries: &mut BTreeMap<String, SnapshotEntry>) {
    for entry in fs::read_dir(folder).expect("read the folder") {
        let entry = entry.expect("read the entry");
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("the entry lies inside the root")
            .display()
            .to_string();
        let metadata = fs::symlink_metadata(&path).expect("inspect the entry");

        if metadata.is_dir() {
            entries.insert(relative, SnapshotEntry::Directory);
            collect_snapshot(root, &path, entries);
        } else if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).expect("read the link");

            entries.insert(relative, SnapshotEntry::Link(target));
        } else {
            let bytes = fs::read(&path).expect("read the file");

            entries.insert(relative, SnapshotEntry::File(bytes));
        }
    }
}

#[cfg(unix)]
#[test]
fn sync_reports_a_stomme_toml_that_is_a_link() {
    use std::os::unix::fs::symlink;

    let root = complete_project();

    fs::remove_file(root.path().join("docs/stomme/shared/coding.md"))
        .expect("remove a synced file that a successful sync would write");
    fs::remove_file(root.path().join("stomme.toml")).expect("remove stomme.toml");
    symlink("missing.toml", root.path().join("stomme.toml")).expect("create the link");

    let before = snapshot(root.path());
    let output = stomme(root.path(), &["sync"]);
    let after = snapshot(root.path());

    assert_eq!(output.status.code(), Some(1), "sync: {output:?}");
    assert_eq!(
        stderr_lines(&output),
        vec!["stomme: stomme.toml is a symbolic link; use a regular file"]
    );
    assert_eq!(after, before, "sync wrote to the project");
}

#[cfg(unix)]
#[test]
fn check_reports_a_stomme_toml_that_is_not_a_regular_file() {
    let root = complete_project();
    let path = root.path().join("stomme.toml");

    fs::remove_file(&path).expect("remove stomme.toml");
    fs::create_dir(&path).expect("create the directory");

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(&output, "stomme.toml", "broken-installation", "directory");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("is not a regular file"),
        "{output:?}"
    );
}

#[cfg(unix)]
#[test]
fn check_reports_a_symlinked_docs_folder() {
    use std::os::unix::fs::symlink;

    let root = complete_project();
    let outside = tempfile::tempdir().expect("create an outside directory");

    fs::rename(root.path().join("docs"), outside.path().join("docs"))
        .expect("move docs outside the project");
    symlink(outside.path().join("docs"), root.path().join("docs")).expect("create the symlink");

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(&output, "docs", "synced-files", "symlinked docs folder");
}

#[cfg(unix)]
#[test]
fn check_reports_a_symlinked_docs_stomme_folder() {
    use std::os::unix::fs::symlink;

    let root = complete_project();
    let outside = tempfile::tempdir().expect("create an outside directory");

    fs::rename(
        root.path().join("docs/stomme"),
        outside.path().join("stomme"),
    )
    .expect("move docs/stomme outside the project");
    symlink(
        outside.path().join("stomme"),
        root.path().join("docs/stomme"),
    )
    .expect("create the symlink");

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(
        &output,
        "docs/stomme",
        "synced-files",
        "symlinked docs/stomme folder",
    );
}

#[test]
fn check_passes_a_project_with_valid_layers() {
    let root = complete_project();

    write_file(
        root.path(),
        "stomme.toml",
        &format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\nlayers = [\"gpui-component\", \"gpui\"]\n",
            env!("CARGO_PKG_VERSION")
        ),
    );
    sync_project(root.path());

    let output = stomme(root.path(), &["check"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_lines(&output), Vec::<String>::new());
}

#[test]
fn check_passes_a_typescript_project_without_the_react_layer() {
    let root = complete_project();

    write_file(
        root.path(),
        "stomme.toml",
        &format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"typescript\"\n",
            env!("CARGO_PKG_VERSION")
        ),
    );
    write_file(root.path(), "src/button.tsx", "");
    sync_project(root.path());

    let output = stomme(root.path(), &["check"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(!stderr.contains("[react-component-file]"), "{stderr}");
}

#[test]
fn check_reports_every_problem_in_one_run() {
    let root = complete_project();

    fs::remove_file(root.path().join("stomme.toml")).expect("remove stomme.toml");
    fs::remove_file(root.path().join("LICENSE")).expect("remove LICENSE");
    write_file(
        root.path(),
        "docs/adr/0001-documentation-scope.md",
        "# Documentation scope\n",
    );

    let output = stomme(root.path(), &["check"]);
    let lines = stderr_lines(&output);
    let tails: Vec<&str> = lines
        .iter()
        .map(|line| line.rsplit_once(' ').map_or("", |(_, rule)| rule))
        .collect();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(tails, ["[no-stomme]", "[required-part]", "[adr-heading]"]);
}

#[test]
fn check_reports_an_adr_it_cannot_read() {
    let root = complete_project();
    let path = "docs/adr/0001-documentation-scope.md";

    fs::write(root.path().join(path), [0xff, 0xfe, b'\n']).expect("write invalid UTF-8");

    let output = stomme(root.path(), &["check"]);
    let lines = stderr_lines(&output);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("stomme: "), "{lines:?}");
    assert!(lines[0].contains(path), "{lines:?}");
}

#[test]
fn stomme_prints_usage_without_the_check_command() {
    let root = complete_project();
    let cases: [&[&str]; 6] = [
        &[],
        &["lint"],
        &["check", "extra"],
        &["check", "--base"],
        &["lint", "--base", "main"],
        &["check", "--reference", "main"],
    ];

    for arguments in cases {
        let output = stomme(root.path(), arguments);

        assert_eq!(output.status.code(), Some(1), "{arguments:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .starts_with("Usage: stomme <check [--base <ref>]|sync|guides <dir>|init --name <name> --language <language> [--layers <layers>] [--roles <roles>]|ocr-coverage <result-file>>"),
            "{arguments:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn check_refuses_a_base_reference_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt as _;

    let root = complete_project();
    let reference = OsStr::from_bytes(b"main-\xff");

    let output = stomme(
        root.path(),
        &[OsStr::new("check"), OsStr::new("--base"), reference],
    );

    assert_refusal(&output, r"main-\xFF");
}

#[cfg(unix)]
#[test]
fn stomme_refuses_a_command_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt as _;

    let root = complete_project();
    let command = OsStr::from_bytes(b"ocr-coverage\xff");

    let output = stomme(root.path(), &[command]);

    assert_refusal(&output, r"ocr-coverage\xFF");
}

#[cfg(unix)]
fn assert_refusal(output: &Output, operand: &str) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{output:?}");

    let lines = stderr_lines(output);

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("stomme: "), "{lines:?}");
    assert!(lines[0].contains(operand), "{lines:?}");
    assert!(lines[0].ends_with("is not valid UTF-8"), "{lines:?}");
}

#[test]
fn check_base_reads_the_named_ref() {
    let root = complete_project();
    let version = env!("CARGO_PKG_VERSION");
    let without_entry = format!(
        "schema = 1\nversion = \"{version}\"\nlanguage = \"rust\"\n\n[roles]\ncli = \"src/domain\"\n"
    );
    let with_entry = format!(
        "{without_entry}\n[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\nkey = \"cli:domain\"\n"
    );

    fs::create_dir_all(root.path().join("src/domain")).expect("create the role folder");
    write_file(root.path(), "stomme.toml", &without_entry);
    git(root.path(), &["init", "-q"]);
    commit(root.path(), "without the entry");
    write_file(root.path(), "stomme.toml", &with_entry);
    commit(root.path(), "with the entry");
    git(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    );
    git(root.path(), &["reset", "--soft", "HEAD^"]);

    let default = stomme(root.path(), &["check"]);
    let named = stomme(root.path(), &["check", "--base", "HEAD"]);

    assert_eq!(default.status.code(), Some(0), "{default:?}");
    assert_eq!(named.status.code(), Some(1), "{named:?}");
    assert!(
        String::from_utf8_lossy(&named.stderr).contains("[violation-growth]"),
        "{named:?}"
    );
}

#[test]
fn check_reports_a_weakened_shared_lint() {
    let root = complete_project();
    let manifest = fs::read_to_string(root.path().join("Cargo.toml")).expect("read Cargo.toml");

    write_file(
        root.path(),
        "Cargo.toml",
        &manifest.replace("todo = \"warn\"", "todo = \"allow\""),
    );

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(&output, "Cargo.toml", "rust-lint-level", "weakened lint");
    assert_eq!(
        stderr_lines(&output),
        [
            "stomme: Cargo.toml: sets lint `clippy::todo` to `allow`; the shared policy needs at least `warn` [rust-lint-level]"
        ]
    );
}

fn adoption_project(name: &str) -> tempfile::TempDir {
    let root = complete_project();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/adoption")
        .join(name);

    copy_tree(&source, root.path());

    let config = root.path().join("stomme.toml");
    let text = fs::read_to_string(&config).expect("read the fixture config");

    fs::write(&config, with_crate_version(&text)).expect("write the fixture config");
    sync_project(root.path());

    root
}

#[test]
fn check_passes_the_tikk_module_table() {
    let root = adoption_project("tikk-modules/pass");

    let output = stomme(root.path(), &["check"]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(stderr_lines(&output), Vec::<String>::new());
}

#[test]
fn check_reports_a_tikk_module_table_break_naming_the_edge() {
    let root = adoption_project("tikk-modules/fail");

    let output = stomme(root.path(), &["check"]);
    let lines = stderr_lines(&output);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("[module-imports]"), "{lines:?}");
    assert!(lines[0].contains("ticket"), "{lines:?}");
    assert!(lines[0].contains("store"), "{lines:?}");
}

#[test]
fn check_passes_the_greppa_split() {
    let root = adoption_project("greppa-split/pass");

    let output = stomme(root.path(), &["check"]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(stderr_lines(&output), Vec::<String>::new());
}

#[test]
fn check_reports_a_greppa_core_gpui_dependency_naming_the_edge() {
    let root = adoption_project("greppa-split/fail");

    let output = stomme(root.path(), &["check"]);
    let lines = stderr_lines(&output);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("[gpui-plain-core]"), "{lines:?}");
    assert!(lines[0].contains("greppa-core"), "{lines:?}");
    assert!(lines[0].contains("gpui-kit"), "{lines:?}");
    assert!(lines[0].contains("layer `gpui`"), "{lines:?}");
}

#[test]
fn guides_rejects_an_empty_directory() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    let output = stomme(root.path(), &["guides", ""]);

    assert_eq!(output.status.code(), Some(1), "guides: {output:?}");
    assert_eq!(
        stderr_lines(&output),
        vec!["stomme: the guides directory is empty"]
    );
    assert_eq!(
        fs::read_dir(root.path())
            .expect("read the temporary directory")
            .count(),
        0
    );
}

#[cfg(unix)]
#[test]
fn guides_writes_a_bundle_to_a_directory_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt as _;

    let root = tempfile::tempdir().expect("create a temporary directory");
    let name = OsStr::from_bytes(b"bundle-\xff");
    let bundle = root.path().join(name);

    let output = stomme(root.path(), &[OsStr::new("guides"), name]);

    assert_eq!(output.status.code(), Some(0), "guides: {output:?}");
    assert!(bundle.join("shared/coding.md").is_file());
}

#[test]
fn guides_writes_a_bundle_without_stomme_toml() {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let bundle = root.path().join("bundle");

    let output = stomme(root.path(), &["guides", "bundle"]);

    assert_eq!(output.status.code(), Some(0), "guides: {output:?}");
    assert!(bundle.join("shared/coding.md").is_file());
    assert!(bundle.join("languages/typescript/guide.md").is_file());
    assert!(bundle.join("layers/herdr/guide.md").is_file());
    assert!(bundle.join(".sync-record.toml").is_file());
    assert!(!root.path().join("stomme.toml").exists());

    let first = fs::read(bundle.join("shared/coding.md")).expect("read the guide");
    let output = stomme(root.path(), &["guides", "bundle"]);

    assert_eq!(output.status.code(), Some(0), "guides: {output:?}");
    assert_eq!(
        fs::read(bundle.join("shared/coding.md")).expect("read the guide"),
        first
    );
}
