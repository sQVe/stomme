use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct Selection {
    name: &'static str,
    language: &'static str,
    layers: &'static str,
    roles: Option<&'static str>,
}

const SELECTIONS: [Selection; 6] = [
    Selection {
        name: "tau",
        language: "typescript",
        layers: "pi",
        roles: None,
    },
    Selection {
        name: "koja",
        language: "typescript",
        layers: "herdr",
        roles: None,
    },
    Selection {
        name: "plugga",
        language: "typescript",
        layers: "react",
        roles: None,
    },
    Selection {
        name: "tikk",
        language: "rust",
        layers: "gpui,gpui-component",
        roles: Some("core,ui,kit,app,cli"),
    },
    Selection {
        name: "greppa",
        language: "rust",
        layers: "gpui,gpui-component",
        roles: Some("core,ui,kit,app,cli"),
    },
    Selection {
        name: "ruta",
        language: "rust",
        layers: "",
        roles: None,
    },
];

fn stomme_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_stomme"))
}

fn init(root: &Path, arguments: &[&str]) -> Output {
    Command::new(stomme_path())
        .arg("init")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("run stomme init")
}

fn init_selection(root: &Path, selection: &Selection) {
    let mut arguments = vec![
        "--name",
        selection.name,
        "--language",
        selection.language,
        "--layers",
        selection.layers,
    ];

    if let Some(roles) = selection.roles {
        arguments.extend(["--roles", roles]);
    }

    let output = init(root, &arguments);

    assert_eq!(
        output.status.code(),
        Some(0),
        "init {}: {output:?}",
        selection.name
    );
}

fn selection_named(name: &str) -> &'static Selection {
    SELECTIONS
        .iter()
        .find(|selection| selection.name == name)
        .expect("the selection exists")
}

fn initialized(name: &str) -> TempDir {
    let root = tempfile::tempdir().expect("create a project folder");

    init_selection(root.path(), selection_named(name));

    root
}

fn path_with_binary_first() -> std::ffi::OsString {
    let binary = stomme_path();
    let binary_folder = binary
        .parent()
        .expect("the binary has a folder")
        .to_path_buf();
    let mut folders = vec![binary_folder];

    folders.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));

    env::join_paths(folders).expect("join the PATH folders")
}

fn run_in_project(root: &Path, program: &str, arguments: &[String]) -> Output {
    Command::new(program)
        .args(arguments)
        .current_dir(root)
        .env("PATH", path_with_binary_first())
        .output()
        .unwrap_or_else(|error| panic!("run {program}: {error}"))
}

fn install(root: &Path) {
    let output = run_in_project(
        root,
        "pnpm",
        &["install".to_string(), "--no-frozen-lockfile".to_string()],
    );

    assert_eq!(output.status.code(), Some(0), "pnpm install: {output:?}");
}

fn full_check_argv(root: &Path) -> Vec<String> {
    let text = fs::read_to_string(root.join(".pi/tau.json")).expect("read .pi/tau.json");
    let config: serde_json::Value = serde_json::from_str(&text).expect("parse .pi/tau.json");
    let argv = config["fullCheck"]["argv"]
        .as_array()
        .expect("fullCheck.argv is an array");

    argv.iter()
        .map(|argument| {
            argument
                .as_str()
                .expect("an argv entry is text")
                .to_string()
        })
        .collect()
}

fn run_full_check(root: &Path) -> Output {
    if root.join("package.json").is_file() {
        install(root);
    }

    let argv = full_check_argv(root);
    let (program, arguments) = argv.split_first().expect("the full check has a program");

    run_in_project(root, program, arguments)
}

fn entries(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root)
        .expect("read the folder")
        .map(|entry| {
            entry
                .expect("read an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    names.sort();

    names
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn package_json(root: &Path) -> serde_json::Value {
    let text = fs::read_to_string(root.join("package.json")).expect("read package.json");

    serde_json::from_str(&text).expect("parse package.json")
}

fn assert_init_refuses_with_an_empty_folder(arguments: &[&str], named: &[&str]) {
    let root = tempfile::tempdir().expect("create a project folder");

    let output = init(root.path(), arguments);
    let message = stderr_text(&output);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    for name in named {
        assert!(message.contains(name), "{name} is missing from: {message}");
    }
    assert_eq!(entries(root.path()), Vec::<String>::new());
}

#[test]
fn init_runs_the_full_check_for_every_selection() {
    for selection in &SELECTIONS {
        let root = tempfile::tempdir().expect("create a project folder");

        init_selection(root.path(), selection);

        let output = run_full_check(root.path());

        assert_eq!(
            output.status.code(),
            Some(0),
            "full check for {}: {output:?}",
            selection.name
        );
    }
}

#[test]
fn init_fails_before_writing_for_an_invalid_layer_set() {
    assert_init_refuses_with_an_empty_folder(
        &[
            "--name",
            "demo",
            "--language",
            "typescript",
            "--layers",
            "gpui",
        ],
        &["gpui", "rust", "typescript"],
    );
}

#[test]
fn init_fails_before_writing_for_a_missing_required_layer() {
    assert_init_refuses_with_an_empty_folder(
        &[
            "--name",
            "demo",
            "--language",
            "rust",
            "--layers",
            "gpui-component",
        ],
        &["gpui"],
    );
}

#[test]
fn init_fails_before_writing_for_an_unknown_role() {
    assert_init_refuses_with_an_empty_folder(
        &[
            "--name",
            "demo",
            "--language",
            "rust",
            "--roles",
            "core,api",
        ],
        &["api"],
    );
}

#[test]
fn init_refuses_a_folder_that_is_not_empty() {
    let root = tempfile::tempdir().expect("create a project folder");

    fs::write(root.path().join("notes.txt"), "keep\n").expect("write a file");

    let output = init(root.path(), &["--name", "demo", "--language", "rust"]);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(entries(root.path()), vec!["notes.txt".to_string()]);
}

#[test]
fn init_accepts_a_folder_with_only_git() {
    let root = tempfile::tempdir().expect("create a project folder");

    fs::create_dir(root.path().join(".git")).expect("create .git");

    let output = init(root.path(), &["--name", "demo", "--language", "rust"]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(root.path().join("stomme.toml").is_file());
}

#[test]
fn herdr_project_bundles_one_main_js() {
    let root = initialized("koja");

    install(root.path());

    let output = run_in_project(
        root.path(),
        "pnpm",
        &["run".to_string(), "build".to_string()],
    );
    let mod_json = fs::read_to_string(root.path().join("mods/koja/mod.json"))
        .expect("read mods/koja/mod.json");
    let manifest: serde_json::Value = serde_json::from_str(&mod_json).expect("parse mod.json");

    assert_eq!(output.status.code(), Some(0), "pnpm run build: {output:?}");
    assert!(root.path().join("mods/koja/main.js").is_file());
    assert_eq!(manifest["name"], "koja");
}

#[test]
fn pi_project_declares_existing_paths() {
    let root = initialized("tau");
    let manifest = package_json(root.path());
    let extensions = manifest["pi"]["extensions"]
        .as_array()
        .expect("pi.extensions is an array");
    let test_source = fs::read_to_string(root.path().join("src/piExtension.test.ts"))
        .expect("read the extension test");

    assert!(!extensions.is_empty(), "{manifest}");
    for extension in extensions {
        let path = extension.as_str().expect("an extension path is text");

        assert!(root.path().join(path).is_file(), "{path} does not exist");
    }

    let extension_source =
        fs::read_to_string(root.path().join("src/piExtension.ts")).expect("read the extension");

    assert!(
        test_source.contains("expect(toolName).toBe('tau')"),
        "{test_source}"
    );
    assert!(
        extension_source.contains("export const toolName = 'tau';"),
        "{extension_source}"
    );
}

#[test]
fn plugga_like_project_has_no_changesets() {
    let root = initialized("plugga");
    let manifest = package_json(root.path());
    let scripts = manifest["scripts"]
        .as_object()
        .expect("scripts is an object");
    let dependencies = ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|section| manifest[section].as_object())
        .flat_map(|section| section.keys());

    assert!(!root.path().join(".changeset").exists());
    assert!(!scripts.contains_key("changeset"));
    for dependency in dependencies {
        assert!(!dependency.contains("changesets"), "{dependency}");
    }
    for entry in fs::read_dir(root.path().join(".github/workflows")).expect("read the workflows") {
        let path = entry.expect("read a workflow").path();
        let workflow = fs::read_to_string(&path).expect("read a workflow");

        assert!(!workflow.contains("changesets"), "{}", path.display());
    }
}

#[test]
fn rust_project_carries_the_layer_bans() {
    let root = initialized("tikk");
    let text = fs::read_to_string(root.path().join("clippy.toml")).expect("read clippy.toml");
    let clippy: toml::Table = toml::from_str(&text).expect("parse clippy.toml");

    assert!(clippy.contains_key("disallowed-methods"), "{text}");
}
