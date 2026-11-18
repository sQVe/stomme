use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use tempfile::TempDir;

use super::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn with_version(fields: &str) -> String {
    format!("schema = 1\nversion = \"{VERSION}\"\n{fields}")
}

fn installation(text: &str) -> Installation {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::write(root.path().join(CONFIG_FILE), text).expect("write stomme.toml");

    read_filesystem(root.path())
}

fn read_filesystem(root: &Path) -> Installation {
    read_with(root, &FileRead::filesystem())
}

fn read_with(root: &Path, file: &FileRead) -> Installation {
    super::read(root, layer::catalog(), file)
}

fn project_with(files: &[(&str, &str)]) -> TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");

    for (path, text) in files {
        let path = root.path().join(path);
        let parent = path.parent().expect("a file has a parent folder");

        fs::create_dir_all(parent).expect("create the folder");
        fs::write(path, text).expect("write the file");
    }

    root
}

fn installation_in(root: &Path, text: &str) -> Installation {
    fs::write(root.join(CONFIG_FILE), text).expect("write stomme.toml");

    read_filesystem(root)
}

fn exceptions(paths: &[&str]) -> String {
    let quoted: Vec<String> = paths.iter().map(|path| format!("\"{path}\"")).collect();

    with_version(&format!(
        "language = \"rust\"\n\n[exceptions]\nextra_docs = [{}]\n",
        quoted.join(", ")
    ))
}

#[test]
fn read_reports_a_toml_syntax_error_as_broken() {
    assert!(matches!(
        installation(&with_version("language = \"rust\n")),
        Installation::Broken(_)
    ));
}

#[test]
fn read_reports_a_layer_of_another_language_as_broken() {
    let installation = installation(&with_version(
        "language = \"typescript\"\nlayers = [\"gpui\"]\n",
    ));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("`gpui`"), "{reason}");
    assert!(reason.contains("rust"), "{reason}");
}

thread_local! {
    static READS: RefCell<usize> = const { RefCell::new(0) };
}

fn read_count() -> usize {
    READS.with(|reads| *reads.borrow())
}

fn recording_read(path: &Path) -> io::Result<String> {
    READS.with(|reads| *reads.borrow_mut() += 1);

    fs::read_to_string(path)
}

fn spy_read(_path: &Path) -> io::Result<String> {
    READS.with(|reads| *reads.borrow_mut() += 1);

    Ok(with_version("language = \"typescript\"\n"))
}

fn denied_inspect(_path: &Path) -> io::Result<fs::Metadata> {
    Err(io::Error::from(io::ErrorKind::PermissionDenied))
}

fn denied_read(_path: &Path) -> io::Result<String> {
    READS.with(|reads| *reads.borrow_mut() += 1);

    Err(io::Error::from(io::ErrorKind::PermissionDenied))
}

fn removed_read(_path: &Path) -> io::Result<String> {
    READS.with(|reads| *reads.borrow_mut() += 1);

    Err(io::Error::from(io::ErrorKind::NotFound))
}

#[cfg(unix)]
#[test]
fn read_classifies_the_config_entry() {
    use std::os::unix::fs::symlink;
    use std::process::Command;

    #[derive(Clone, Copy)]
    enum Entry {
        Missing,
        Regular,
        LinkInside,
        LinkOutside,
        DanglingLink,
        LinkLoop,
        LinkThroughFile,
        Fifo,
        Directory,
        InspectDenied,
        ReadDenied,
        RemovedAfterInspect,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Expected {
        NoStomme,
        Valid,
        SymbolicLink,
        NotRegularFile,
        Unreadable(io::ErrorKind),
    }

    fn write_entry(root: &TempDir, outside: &TempDir, text: &str, entry: Entry) {
        let path = root.path().join(CONFIG_FILE);

        match entry {
            Entry::Missing | Entry::InspectDenied => {}
            Entry::Regular | Entry::ReadDenied | Entry::RemovedAfterInspect => {
                fs::write(path, text).expect("write stomme.toml");
            }
            Entry::LinkInside => {
                fs::write(root.path().join("shared.toml"), text).expect("write the target");
                symlink("shared.toml", path).expect("create the link");
            }
            Entry::LinkOutside => {
                fs::write(outside.path().join(CONFIG_FILE), text).expect("write the target");
                symlink(outside.path().join(CONFIG_FILE), path).expect("create the link");
            }
            Entry::DanglingLink => symlink("missing.toml", path).expect("create the link"),
            Entry::LinkLoop => symlink(CONFIG_FILE, path).expect("create the link loop"),
            Entry::LinkThroughFile => {
                fs::write(root.path().join("regular"), "contents").expect("write the file");
                symlink("regular/child.toml", path).expect("create the link");
            }
            Entry::Fifo => {
                let status = Command::new("mkfifo")
                    .arg(&path)
                    .status()
                    .expect("run mkfifo");

                assert!(status.success(), "mkfifo: {status:?}");
            }
            Entry::Directory => fs::create_dir(path).expect("create the directory"),
        }
    }

    fn file_read(entry: Entry) -> FileRead {
        match entry {
            Entry::Regular => FileRead {
                inspect: inspect_entry,
                read: recording_read,
            },
            Entry::InspectDenied => FileRead {
                inspect: denied_inspect,
                read: spy_read,
            },
            Entry::ReadDenied => FileRead {
                inspect: inspect_entry,
                read: denied_read,
            },
            Entry::RemovedAfterInspect => FileRead {
                inspect: inspect_entry,
                read: removed_read,
            },
            _ => FileRead {
                inspect: inspect_entry,
                read: spy_read,
            },
        }
    }

    let cases = [
        ("missing entry", Entry::Missing, Expected::NoStomme, 0),
        ("regular file", Entry::Regular, Expected::Valid, 1),
        (
            "link inside the project",
            Entry::LinkInside,
            Expected::SymbolicLink,
            0,
        ),
        (
            "link outside the project",
            Entry::LinkOutside,
            Expected::SymbolicLink,
            0,
        ),
        (
            "dangling link",
            Entry::DanglingLink,
            Expected::SymbolicLink,
            0,
        ),
        ("link loop", Entry::LinkLoop, Expected::SymbolicLink, 0),
        (
            "link through a file",
            Entry::LinkThroughFile,
            Expected::SymbolicLink,
            0,
        ),
        ("fifo", Entry::Fifo, Expected::NotRegularFile, 0),
        ("directory", Entry::Directory, Expected::NotRegularFile, 0),
        (
            "inspect denied",
            Entry::InspectDenied,
            Expected::Unreadable(io::ErrorKind::PermissionDenied),
            0,
        ),
        (
            "read denied",
            Entry::ReadDenied,
            Expected::Unreadable(io::ErrorKind::PermissionDenied),
            1,
        ),
        (
            "entry removed after inspect",
            Entry::RemovedAfterInspect,
            Expected::Unreadable(io::ErrorKind::NotFound),
            1,
        ),
    ];

    for (name, entry, expected, reads) in cases {
        let root = tempfile::tempdir().expect("create a temporary directory");
        let outside = tempfile::tempdir().expect("create an outside directory");
        let text = with_version("language = \"typescript\"\n");

        write_entry(&root, &outside, &text, entry);
        READS.with(|count| *count.borrow_mut() = 0);

        let installation = read_with(root.path(), &file_read(entry));
        let actual_reads = read_count();
        let actual = match installation {
            Installation::NoStomme => Expected::NoStomme,
            Installation::Valid(_) => Expected::Valid,
            Installation::Broken(Reason::SymbolicLink) => Expected::SymbolicLink,
            Installation::Broken(Reason::NotRegularFile) => Expected::NotRegularFile,
            Installation::Broken(Reason::Unreadable { source }) => {
                Expected::Unreadable(source.kind())
            }
            Installation::Broken(Reason::Invalid(reason)) => {
                panic!("{name}: unexpected invalid config: {reason}");
            }
        };

        assert_eq!(actual, expected, "{name}");
        assert_eq!(actual_reads, reads, "{name}");
    }
}

#[test]
fn read_accepts_a_schema_one_file() {
    let text =
        with_version("language = \"rust\"\nlayers = [\"gpui\"]\n\n[roles]\ncli = \"crates/cli\"\n");
    let Installation::Valid(selection) = installation(&text) else {
        panic!("expected a valid installation");
    };

    assert_eq!(
        selection,
        Selection {
            version: VERSION.to_string(),
            language: Language::Rust,
            layers: BTreeSet::from(["gpui".to_string()]),
            roles: BTreeMap::from([("cli".to_string(), "crates/cli".to_string())]),
            violations: Vec::new(),
            extra_docs: Vec::new(),
            modules: BTreeMap::new(),
        }
    );
}

#[test]
fn read_defaults_to_no_layers_and_no_roles() {
    let Installation::Valid(selection) = installation(&with_version("language = \"typescript\"\n"))
    else {
        panic!("expected a valid installation");
    };

    assert_eq!(
        selection,
        Selection {
            version: VERSION.to_string(),
            language: Language::TypeScript,
            layers: BTreeSet::new(),
            roles: BTreeMap::new(),
            violations: Vec::new(),
            extra_docs: Vec::new(),
            modules: BTreeMap::new(),
        }
    );
}

#[test]
fn read_gives_the_same_selection_for_layers_in_either_order() {
    let forward = installation(&with_version(
        "language = \"rust\"\nlayers = [\"gpui\", \"gpui-component\"]\n",
    ));
    let backward = installation(&with_version(
        "language = \"rust\"\nlayers = [\"gpui-component\", \"gpui\"]\n",
    ));

    let Installation::Valid(forward) = forward else {
        panic!("expected a valid installation, got {forward:?}");
    };
    let Installation::Valid(backward) = backward else {
        panic!("expected a valid installation, got {backward:?}");
    };

    assert_eq!(forward, backward);
}

#[test]
fn read_reports_a_leftover_kind_with_the_fields_that_replace_it() {
    let installation = installation(&with_version("kind = \"rust-cli\"\n"));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    for field in ["`language`", "`layers`", "`[roles]`"] {
        assert!(reason.contains(field), "{field}: {reason}");
    }
}

#[test]
fn read_reports_a_bad_layer_set_naming_the_layer() {
    let cases = [
        (
            "unknown layer",
            "language = \"rust\"\nlayers = [\"egui\"]\n",
            "`egui`",
        ),
        (
            "duplicate layer",
            "language = \"rust\"\nlayers = [\"gpui\", \"gpui\"]\n",
            "`gpui`",
        ),
        (
            "missing required layer",
            "language = \"rust\"\nlayers = [\"gpui-component\"]\n",
            "`gpui`",
        ),
    ];

    for (case, text, layer) in cases {
        let installation = installation(&with_version(text));

        let Installation::Broken(Reason::Invalid(reason)) = installation else {
            panic!("{case}: expected a broken installation, got {installation:?}");
        };
        assert!(reason.contains(layer), "{case}: {reason}");
    }
}

#[test]
fn read_reports_a_version_other_than_the_binary_version_as_broken() {
    let installation = installation("schema = 1\nversion = \"9.9.9\"\nlanguage = \"rust\"\n");

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("9.9.9"), "{reason}");
    assert!(reason.contains(VERSION), "{reason}");
}

#[test]
fn read_reports_a_file_that_does_not_match_schema_one_as_broken() {
    let cases = [
        (
            "unknown field",
            with_version("language = \"rust\"\nname = \"x\"\n"),
        ),
        (
            "missing version",
            "schema = 1\nlanguage = \"rust\"\n".to_string(),
        ),
        ("missing language", with_version("")),
        ("unknown language", with_version("language = \"go\"\n")),
        (
            "layers not a list",
            with_version("language = \"rust\"\nlayers = \"gpui\"\n"),
        ),
        (
            "role not a path",
            with_version("language = \"rust\"\n[roles]\ncli = 1\n"),
        ),
        (
            "schema 2",
            format!("schema = 2\nversion = \"{VERSION}\"\nlanguage = \"rust\"\n"),
        ),
        (
            "missing schema",
            format!("version = \"{VERSION}\"\nlanguage = \"rust\"\n"),
        ),
        (
            "text schema",
            format!("schema = \"1\"\nversion = \"{VERSION}\"\nlanguage = \"rust\"\n"),
        ),
    ];

    for (case, text) in cases {
        assert!(
            matches!(installation(&text), Installation::Broken(_)),
            "{case}"
        );
    }
}

#[test]
fn read_accepts_extra_docs_under_docs() {
    let root = project_with(&[("docs/development.md", "# Development\n")]);
    let installation = installation_in(root.path(), &exceptions(&["docs/development.md"]));
    let Installation::Valid(selection) = installation else {
        panic!("expected a valid installation");
    };

    assert_eq!(
        selection,
        Selection {
            version: VERSION.to_string(),
            language: Language::Rust,
            layers: BTreeSet::new(),
            roles: BTreeMap::new(),
            violations: Vec::new(),
            extra_docs: vec!["docs/development.md".to_string()],
            modules: BTreeMap::new(),
        }
    );
}

#[test]
fn read_reports_an_extra_doc_that_does_not_exist() {
    let root = project_with(&[]);
    let installation = installation_in(root.path(), &exceptions(&["docs/missing.md"]));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/missing.md"), "{reason}");
}

#[test]
fn read_reports_an_extra_doc_inside_docs_stomme() {
    let root = project_with(&[("docs/stomme/guide.md", "# Guide\n")]);
    let installation = installation_in(root.path(), &exceptions(&["docs/stomme/guide.md"]));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/stomme/guide.md"), "{reason}");
}

#[test]
fn read_reports_an_extra_doc_outside_docs() {
    let root = project_with(&[("README.md", "# Readme\n")]);
    let installation = installation_in(root.path(), &exceptions(&["README.md"]));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("README.md"), "{reason}");
}

#[test]
fn read_reports_an_absolute_extra_doc() {
    let root = project_with(&[]);
    let installation = installation_in(root.path(), &exceptions(&["/etc/passwd"]));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("/etc/passwd"), "{reason}");
}

#[test]
fn read_reports_an_extra_doc_with_a_parent_part() {
    let root = project_with(&[("README.md", "# Readme\n")]);
    let installation = installation_in(root.path(), &exceptions(&["docs/../README.md"]));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/../README.md"), "{reason}");
}

#[cfg(unix)]
#[test]
fn read_reports_an_extra_doc_that_resolves_outside_docs() {
    use std::os::unix::fs::symlink;

    let root = project_with(&[("docs/development.md", "# Development\n")]);
    let outside = tempfile::tempdir().expect("create an outside directory");
    let target = outside.path().join("outside.md");

    fs::write(&target, "# Outside\n").expect("write the outside file");
    symlink(&target, root.path().join("docs/link.md")).expect("create the symlink");

    let installation = installation_in(root.path(), &exceptions(&["docs/link.md"]));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/link.md"), "{reason}");
}

#[test]
fn read_reports_an_extra_doc_that_aliases_docs_stomme_with_a_dot() {
    let root = project_with(&[("docs/stomme/shared/coding.md", "# Coding\n")]);
    let path = "docs/./stomme/shared/coding.md";
    let installation = installation_in(root.path(), &exceptions(&[path]));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains(path), "{reason}");
}

#[cfg(unix)]
#[test]
fn read_reports_an_extra_doc_symlinked_into_docs_stomme() {
    use std::os::unix::fs::symlink;

    let root = project_with(&[("docs/stomme/shared/coding.md", "# Coding\n")]);

    symlink(
        root.path().join("docs/stomme/shared/coding.md"),
        root.path().join("docs/link.md"),
    )
    .expect("create the symlink");

    let installation = installation_in(root.path(), &exceptions(&["docs/link.md"]));

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/link.md"), "{reason}");
}

#[test]
fn read_reports_an_unknown_exception_field() {
    let root = project_with(&[]);
    let text = with_version("language = \"rust\"\n\n[exceptions]\nother = []\n");
    let installation = installation_in(root.path(), &text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("other"), "{reason}");
}

#[test]
fn read_accepts_recorded_violations() {
    let text = with_version(
        "language = \"rust\"\n\n[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\nkey = \"cli:domain\"\n",
    );

    let Installation::Valid(selection) = installation(&text) else {
        panic!("expected a valid installation");
    };
    assert_eq!(
        selection.violations,
        vec![Violation {
            path: "src/domain".to_string(),
            rule: Rule::RoleFolder,
            key: "cli:domain".to_string(),
        }]
    );
}

#[test]
fn read_rejects_bad_violation_entries() {
    let cases = [
        ("not a list", "violations = \"x\"\n", "not a list of tables"),
        (
            "entry not a table",
            "violations = [\"x\"]\n",
            "entry 1 that is not a table",
        ),
        (
            "missing path",
            "[[violations]]\nrule = \"role-folder\"\nkey = \"cli:domain\"\n",
            "without `path`",
        ),
        (
            "path not a string",
            "[[violations]]\npath = 1\nrule = \"role-folder\"\nkey = \"cli:domain\"\n",
            "whose `path` is not a string",
        ),
        (
            "missing rule",
            "[[violations]]\npath = \"src/domain\"\nkey = \"domain\"\n",
            "without `rule`",
        ),
        (
            "rule not a string",
            "[[violations]]\npath = \"src/domain\"\nrule = 1\nkey = \"domain\"\n",
            "whose `rule` is not a string",
        ),
        (
            "missing key",
            "[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\n",
            "without `key`",
        ),
        (
            "key not a string",
            "[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\nkey = 1\n",
            "whose `key` is not a string",
        ),
        (
            "unknown rule",
            "[[violations]]\npath = \"src/domain\"\nrule = \"unknown-rule\"\nkey = \"domain\"\n",
            "unknown-rule",
        ),
        (
            "unknown field",
            "[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\nkey = \"domain\"\nextra = \"x\"\n",
            "unknown field `extra`",
        ),
        (
            "duplicate entry",
            "[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\nkey = \"cli:domain\"\n\n[[violations]]\npath = \"src/domain\"\nrule = \"role-folder\"\nkey = \"cli:domain\"\n",
            "twice",
        ),
    ];

    for (case, entry, named) in cases {
        let text = with_version(&format!("language = \"rust\"\n\n{entry}"));
        let installation = installation(&text);

        let Installation::Broken(Reason::Invalid(reason)) = installation else {
            panic!("{case}: expected a broken installation, got {installation:?}");
        };
        assert!(reason.contains(named), "{case}: {reason}");
    }
}

#[test]
fn read_reports_an_import_of_a_module_without_a_row() {
    let text = with_version(
        "language = \"typescript\"\n\n[modules.config]\nentry = \"src/config.ts\"\nimports = [\"errors\"]\n",
    );
    let installation = installation(&text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("`config`"), "{reason}");
    assert!(reason.contains("`errors`"), "{reason}");
}

#[test]
fn read_accepts_a_closed_module_table() {
    let text = with_version(
        "language = \"typescript\"\n\n[modules.errors]\nentry = \"src/errors.ts\"\n\n[modules.config]\nentry = \"src/config/config.ts\"\npublic = [\"src/config/models.ts\"]\nimports = [\"errors\"]\n",
    );

    assert!(matches!(installation(&text), Installation::Valid(_)));
}

#[test]
fn read_reports_an_absolute_module_path() {
    let text = with_version("language = \"typescript\"\n\n[modules.a]\nentry = \"/src/a.ts\"\n");
    let installation = installation(&text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("/src/a.ts"), "{reason}");
}

#[test]
fn read_reports_a_module_path_with_a_parent_part() {
    let text = with_version(
        "language = \"typescript\"\n\n[modules.a]\nentry = \"src/a.ts\"\npublic = [\"src/../a.ts\"]\n",
    );
    let installation = installation(&text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("src/../a.ts"), "{reason}");
}

#[test]
fn read_reports_an_unknown_module_row_field() {
    let text = with_version(
        "language = \"typescript\"\n\n[modules.a]\nentry = \"src/a.ts\"\nexports = []\n",
    );
    let installation = installation(&text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("exports"), "{reason}");
}

#[test]
fn shared_module_table_fixtures_accept_and_reject_by_folder() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/module-table");

    for (cases, accepts) in [("accept", true), ("reject", false)] {
        let cases = module_table_cases(&folder.join(cases));

        assert!(!cases.is_empty(), "{cases:?} has no cases");
        for case in cases {
            let root = tempfile::tempdir().expect("create a temporary directory");

            let config = fs::read_to_string(case.join(CONFIG_FILE)).expect("read stomme.toml");
            let config = crate::test_support::with_crate_version(&config);

            fs::write(root.path().join(CONFIG_FILE), config).expect("write stomme.toml");

            let installation = read_filesystem(root.path());
            let case = case.display();

            if accepts {
                assert!(
                    matches!(installation, Installation::Valid(_)),
                    "{case}: {installation:?}"
                );
            } else {
                assert!(
                    matches!(installation, Installation::Broken(_)),
                    "{case}: {installation:?}"
                );
            }
        }
    }
}

fn module_table_cases(folder: &Path) -> Vec<std::path::PathBuf> {
    fs::read_dir(folder)
        .expect("list the module table fixtures")
        .map(|entry| entry.expect("read a module table fixture").path())
        .filter(|path| path.is_dir())
        .collect()
}

#[test]
fn read_reports_a_rust_module_path_that_is_not_a_rust_file() {
    let root = project_with(&[("src/a.rs", "")]);
    let text = with_version("language = \"rust\"\n\n[modules.a]\nentry = \"src/a.txt\"\n");
    let installation = installation_in(root.path(), &text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("src/a.txt"), "{reason}");
    assert!(reason.contains(".rs"), "{reason}");
}

#[test]
fn read_reports_a_rust_module_path_without_a_src_folder() {
    let root = project_with(&[("a.rs", "")]);
    let text = with_version("language = \"rust\"\n\n[modules.a]\nentry = \"a.rs\"\n");
    let installation = installation_in(root.path(), &text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("a.rs"), "{reason}");
    assert!(reason.contains("src"), "{reason}");
}

#[test]
fn read_reports_a_rust_module_path_that_is_not_a_file() {
    let root = project_with(&[]);
    let text = with_version("language = \"rust\"\n\n[modules.a]\nentry = \"src/a.rs\"\n");
    let installation = installation_in(root.path(), &text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("src/a.rs"), "{reason}");
    assert!(reason.contains("existing file"), "{reason}");
}

#[test]
fn read_reports_two_modules_with_the_same_entry() {
    let root = project_with(&[("src/a.rs", "")]);
    let text = with_version(
        "language = \"rust\"\n\n[modules.a]\nentry = \"src/a.rs\"\n\n[modules.b]\nentry = \"src/a.rs\"\n",
    );
    let installation = installation_in(root.path(), &text);

    let Installation::Broken(Reason::Invalid(reason)) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("`a`"), "{reason}");
    assert!(reason.contains("`b`"), "{reason}");
    assert!(reason.contains("src/a.rs"), "{reason}");
}
