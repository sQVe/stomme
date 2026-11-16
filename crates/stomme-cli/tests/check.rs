use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../src/test_support.rs"]
mod test_support;

use test_support::{complete_project, write_file};

fn sync_project(root: &Path) {
    let output = stomme(root, &["sync"]);

    assert_eq!(output.status.code(), Some(0), "sync: {output:?}");
}

fn stomme(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stomme"))
        .args(arguments)
        .current_dir(root)
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
fn check_reports_an_extra_doc_inside_docs_stomme() {
    let root = complete_project();

    write_file(
        root.path(),
        "stomme.toml",
        &format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n\n[exceptions]\nextra_docs = [\"docs/stomme/shared/coding.md\"]\n",
            env!("CARGO_PKG_VERSION")
        ),
    );

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(
        &output,
        "stomme.toml",
        "broken-installation",
        "extra doc inside docs/stomme",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("docs/stomme/shared/coding.md"),
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

#[test]
fn check_reports_an_extra_doc_that_aliases_docs_stomme_with_a_dot() {
    let root = complete_project();
    let path = "docs/./stomme/shared/coding.md";

    write_file(
        root.path(),
        "stomme.toml",
        &format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n\n[exceptions]\nextra_docs = [\"{path}\"]\n",
            env!("CARGO_PKG_VERSION")
        ),
    );

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(
        &output,
        "stomme.toml",
        "broken-installation",
        "dotted extra doc alias",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(path),
        "{output:?}"
    );
}

#[cfg(unix)]
#[test]
fn check_reports_an_extra_doc_symlinked_into_docs_stomme() {
    use std::os::unix::fs::symlink;

    let root = complete_project();

    symlink(
        root.path().join("docs/stomme/shared/coding.md"),
        root.path().join("docs/link.md"),
    )
    .expect("create the symlink");
    write_file(
        root.path(),
        "stomme.toml",
        &format!(
            "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n\n[exceptions]\nextra_docs = [\"docs/link.md\"]\n",
            env!("CARGO_PKG_VERSION")
        ),
    );

    let output = stomme(root.path(), &["check"]);

    assert_single_problem(
        &output,
        "stomme.toml",
        "broken-installation",
        "symlinked extra doc alias",
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("docs/link.md"),
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
    let cases: [&[&str]; 4] = [&[], &["lint"], &["check", "extra"], &["check", "--base"]];

    for arguments in cases {
        let output = stomme(root.path(), arguments);

        assert_eq!(output.status.code(), Some(1), "{arguments:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .starts_with("Usage: stomme <check [--base <ref>]|sync>"),
            "{arguments:?}"
        );
    }
}

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("run git");

    assert!(output.status.success(), "{arguments:?}: {output:?}");
}

#[test]
fn check_base_reads_the_named_ref() {
    let root = complete_project();
    let version = env!("CARGO_PKG_VERSION");
    let without_entry = format!(
        "schema = 1\nversion = \"{version}\"\nlanguage = \"rust\"\n\n[roles]\ncli = \"src/domain\"\n"
    );
    let with_entry = format!(
        "{without_entry}\n[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\nkey = \"domain\"\n"
    );

    fs::create_dir_all(root.path().join("src/domain")).expect("create the role folder");
    write_file(root.path(), "stomme.toml", &without_entry);
    git(root.path(), &["init", "-q"]);
    git(root.path(), &["add", "-A"]);
    git(
        root.path(),
        &[
            "-c",
            "user.name=Stomme Test",
            "-c",
            "user.email=stomme@example.com",
            "commit",
            "-qm",
            "without the entry",
        ],
    );
    write_file(root.path(), "stomme.toml", &with_entry);
    git(root.path(), &["add", "-A"]);
    git(
        root.path(),
        &[
            "-c",
            "user.name=Stomme Test",
            "-c",
            "user.email=stomme@example.com",
            "commit",
            "-qm",
            "with the entry",
        ],
    );
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
