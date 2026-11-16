use std::collections::{BTreeMap, BTreeSet};
use std::fs;
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

    read(root.path(), layer::catalog())
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

    read(root, layer::catalog())
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

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("`gpui`"), "{reason}");
    assert!(reason.contains("rust"), "{reason}");
}

#[test]
fn read_reports_a_missing_file_as_no_stomme() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    assert_eq!(read(root.path(), layer::catalog()), Installation::NoStomme);
}

#[test]
fn read_accepts_a_schema_one_file() {
    let text =
        with_version("language = \"rust\"\nlayers = [\"gpui\"]\n\n[roles]\ncli = \"crates/cli\"\n");

    assert_eq!(
        installation(&text),
        Installation::Valid(Selection {
            version: VERSION.to_string(),
            language: Language::Rust,
            layers: BTreeSet::from(["gpui".to_string()]),
            roles: BTreeMap::from([("cli".to_string(), "crates/cli".to_string())]),
            violations: Vec::new(),
            extra_docs: Vec::new(),
        })
    );
}

#[test]
fn read_defaults_to_no_layers_and_no_roles() {
    assert_eq!(
        installation(&with_version("language = \"typescript\"\n")),
        Installation::Valid(Selection {
            version: VERSION.to_string(),
            language: Language::TypeScript,
            layers: BTreeSet::new(),
            roles: BTreeMap::new(),
            violations: Vec::new(),
            extra_docs: Vec::new(),
        })
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

    assert!(matches!(forward, Installation::Valid(_)), "{forward:?}");
    assert_eq!(forward, backward);
}

#[test]
fn read_reports_a_leftover_kind_with_the_fields_that_replace_it() {
    let installation = installation(&with_version("kind = \"rust-cli\"\n"));

    let Installation::Broken(reason) = installation else {
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

        let Installation::Broken(reason) = installation else {
            panic!("{case}: expected a broken installation, got {installation:?}");
        };
        assert!(reason.contains(layer), "{case}: {reason}");
    }
}

#[test]
fn read_reports_a_version_other_than_the_binary_version_as_broken() {
    let installation = installation("schema = 1\nversion = \"9.9.9\"\nlanguage = \"rust\"\n");

    let Installation::Broken(reason) = installation else {
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
fn read_reports_a_file_it_cannot_read_as_broken() {
    let root = tempfile::tempdir().expect("create a temporary directory");

    fs::create_dir(root.path().join(CONFIG_FILE)).expect("create a stomme.toml directory");

    assert!(matches!(
        read(root.path(), layer::catalog()),
        Installation::Broken(_)
    ));
}

#[test]
fn read_accepts_extra_docs_under_docs() {
    let root = project_with(&[("docs/development.md", "# Development\n")]);
    let installation = installation_in(root.path(), &exceptions(&["docs/development.md"]));

    assert_eq!(
        installation,
        Installation::Valid(Selection {
            version: VERSION.to_string(),
            language: Language::Rust,
            layers: BTreeSet::new(),
            roles: BTreeMap::new(),
            violations: Vec::new(),
            extra_docs: vec!["docs/development.md".to_string()],
        })
    );
}

#[test]
fn read_reports_an_extra_doc_that_does_not_exist() {
    let root = project_with(&[]);
    let installation = installation_in(root.path(), &exceptions(&["docs/missing.md"]));

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/missing.md"), "{reason}");
}

#[test]
fn read_reports_an_extra_doc_inside_docs_stomme() {
    let root = project_with(&[("docs/stomme/guide.md", "# Guide\n")]);
    let installation = installation_in(root.path(), &exceptions(&["docs/stomme/guide.md"]));

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/stomme/guide.md"), "{reason}");
}

#[test]
fn read_reports_an_extra_doc_outside_docs() {
    let root = project_with(&[("README.md", "# Readme\n")]);
    let installation = installation_in(root.path(), &exceptions(&["README.md"]));

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("README.md"), "{reason}");
}

#[test]
fn read_reports_an_absolute_extra_doc() {
    let root = project_with(&[]);
    let installation = installation_in(root.path(), &exceptions(&["/etc/passwd"]));

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("/etc/passwd"), "{reason}");
}

#[test]
fn read_reports_an_extra_doc_with_a_parent_part() {
    let root = project_with(&[("README.md", "# Readme\n")]);
    let installation = installation_in(root.path(), &exceptions(&["docs/../README.md"]));

    let Installation::Broken(reason) = installation else {
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

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/link.md"), "{reason}");
}

#[test]
fn read_reports_an_extra_doc_that_aliases_docs_stomme_with_a_dot() {
    let root = project_with(&[("docs/stomme/shared/coding.md", "# Coding\n")]);
    let path = "docs/./stomme/shared/coding.md";
    let installation = installation_in(root.path(), &exceptions(&[path]));

    let Installation::Broken(reason) = installation else {
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

    let Installation::Broken(reason) = installation else {
        panic!("expected a broken installation, got {installation:?}");
    };
    assert!(reason.contains("docs/link.md"), "{reason}");
}

#[test]
fn read_reports_an_unknown_exception_field() {
    let root = project_with(&[]);
    let text = with_version("language = \"rust\"\n\n[exceptions]\nother = []\n");
    let installation = installation_in(root.path(), &text);

    let Installation::Broken(reason) = installation else {
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

        let Installation::Broken(reason) = installation else {
            panic!("{case}: expected a broken installation, got {installation:?}");
        };
        assert!(reason.contains(named), "{case}: {reason}");
    }
}
