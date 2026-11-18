use std::collections::BTreeSet;
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

fn project_root() -> TempDir {
    let base = env::temp_dir();
    let inside_workspace = base
        .ancestors()
        .any(|folder| folder.join("pnpm-workspace.yaml").is_file());
    let root = if inside_workspace {
        PathBuf::from("/tmp")
    } else {
        base
    };

    tempfile::Builder::new()
        .prefix("stomme-init-")
        .tempdir_in(root)
        .expect("create a project folder outside any pnpm workspace")
}

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
    let root = project_root();

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
    let root = project_root();

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
        let root = project_root();

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
fn init_fails_before_writing_for_a_rust_xtask_name() {
    assert_init_refuses_with_an_empty_folder(
        &["--name", "xtask", "--language", "rust"],
        &["xtask", "collides"],
    );
}

#[test]
fn init_fails_before_writing_for_a_name_over_the_limit() {
    let name = "a".repeat(215);

    for language in ["rust", "typescript"] {
        assert_init_refuses_with_an_empty_folder(
            &["--name", name.as_str(), "--language", language],
            &["longer than 214 characters"],
        );
    }
}

#[test]
fn init_refuses_a_folder_that_is_not_empty() {
    let root = project_root();

    fs::write(root.path().join("notes.txt"), "keep\n").expect("write a file");

    let output = init(root.path(), &["--name", "demo", "--language", "rust"]);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(entries(root.path()), vec!["notes.txt".to_string()]);
}

#[test]
fn init_accepts_a_folder_with_only_git() {
    let root = project_root();

    fs::create_dir(root.path().join(".git")).expect("create .git");

    let output = init(root.path(), &["--name", "demo", "--language", "rust"]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(root.path().join("stomme.toml").is_file());
}

#[test]
fn herdr_starter_mod_passes_herdr_check() {
    let root = initialized("koja");

    install(root.path());

    let build = run_in_project(
        root.path(),
        "pnpm",
        &["run".to_string(), "build".to_string()],
    );

    assert_eq!(build.status.code(), Some(0), "pnpm run build: {build:?}");

    let check = run_in_project(
        root.path(),
        "herdr-app",
        &["check".to_string(), "mods/koja".to_string()],
    );

    assert_eq!(check.status.code(), Some(0), "herdr-app check: {check:?}");
}

#[test]
fn herdr_full_check_fails_when_herdr_rejects_the_mod() {
    let root = initialized("koja");
    let mod_folder = root.path().join("mods/koja");

    fs::remove_file(mod_folder.join("greeting.test.ts")).expect("remove greeting.test.ts");
    fs::remove_file(mod_folder.join("greeting.ts")).expect("remove greeting.ts");
    fs::write(
        mod_folder.join("main.ts"),
        "export const modName = 'koja';\n",
    )
    .expect("replace main.ts");

    let output = run_full_check(root.path());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        stderr_text(&output)
    );

    assert_ne!(output.status.code(), Some(0), "{output:?}");
    assert!(text.contains("extends View"), "{text}");
}

#[test]
fn herdr_full_check_fails_when_the_build_fails_after_a_good_bundle() {
    let root = initialized("koja");

    install(root.path());

    let build = run_in_project(
        root.path(),
        "pnpm",
        &["run".to_string(), "build".to_string()],
    );

    assert_eq!(build.status.code(), Some(0), "pnpm run build: {build:?}");

    let mod_path = root.path().join("mods/koja/main.ts");
    let source = fs::read_to_string(&mod_path).expect("read mods/koja/main.ts");
    let source = format!(
        "{source}\n// @ts-expect-error The module does not exist, so the bundle cannot build.\nexport {{ missing }} from './missing.ts';\n"
    );

    fs::write(&mod_path, source).expect("append the unresolvable export");

    let output = run_full_check(root.path());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        stderr_text(&output)
    );

    assert!(
        root.path().join("mods/koja/main.js").is_file(),
        "the earlier bundle is still on disk"
    );
    assert!(text.contains("Could not resolve"), "{text}");
    assert_ne!(output.status.code(), Some(0), "{output:?}");
    assert!(!text.contains("check passed"), "{text}");
}

#[test]
fn herdr_build_marks_every_host_module_external() {
    let root = initialized("koja");

    install(root.path());

    let declarations = fs::read_to_string(root.path().join("mods/koja/gpui-kit.d.ts"))
        .expect("read gpui-kit.d.ts");
    let declared: BTreeSet<&str> = declarations
        .lines()
        .filter_map(|line| line.strip_prefix("declare module \""))
        .filter_map(|rest| rest.split('"').next())
        .collect();
    let manifest = package_json(root.path());
    let build = manifest["scripts"]["build"]
        .as_str()
        .expect("scripts.build is text");
    let external: BTreeSet<&str> = build
        .split("--external ")
        .skip(1)
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();

    assert!(!declared.is_empty(), "{declarations}");
    assert_eq!(declared, external);
}

#[test]
fn herdr_build_keeps_gpui_base_external() {
    let root = initialized("koja");

    install(root.path());

    let mod_path = root.path().join("mods/koja/main.ts");
    let source = fs::read_to_string(&mod_path).expect("read mods/koja/main.ts");
    let source = format!(
        "{source}\nimport {{ v_flex }} from 'gpui-base';\n\nexport const column = v_flex;\n"
    );

    fs::write(&mod_path, source).expect("append the gpui-base import");

    let build = run_in_project(
        root.path(),
        "pnpm",
        &["run".to_string(), "build".to_string()],
    );

    assert_eq!(build.status.code(), Some(0), "pnpm run build: {build:?}");

    let bundle =
        fs::read_to_string(root.path().join("mods/koja/main.js")).expect("read mods/koja/main.js");
    let check = run_in_project(
        root.path(),
        "herdr-app",
        &["check".to_string(), "mods/koja".to_string()],
    );
    let mod_json = fs::read_to_string(root.path().join("mods/koja/mod.json"))
        .expect("read mods/koja/mod.json");
    let manifest: serde_json::Value = serde_json::from_str(&mod_json).expect("parse mod.json");

    assert!(bundle.contains("\"gpui-base\""), "{bundle}");
    assert_eq!(check.status.code(), Some(0), "herdr-app check: {check:?}");
    assert_eq!(manifest["name"], "koja");
}

#[test]
fn herdr_install_writes_ignored_declarations() {
    let root = initialized("koja");

    install(root.path());

    let gitignore = fs::read_to_string(root.path().join(".gitignore")).expect("read .gitignore");

    assert!(root.path().join("mods/koja/gpui-kit.d.ts").is_file());
    assert!(!root.path().join("mods/koja/jsconfig.json").exists());
    assert!(
        gitignore
            .lines()
            .any(|line| line == "mods/**/gpui-kit.d.ts"),
        "{gitignore}"
    );
}

#[test]
fn herdr_docs_link_the_development_setup_before_the_steps() {
    let root = initialized("koja");
    let docs = fs::read_to_string(root.path().join("docs/herdr.md")).expect("read docs/herdr.md");
    let link = docs
        .find("](development.md#set-up)")
        .expect("link the development setup");
    let first_step = docs.find("\n1. ").expect("number the setup steps");

    assert!(link < first_step, "{docs}");
    assert!(!docs.contains("mise trust"), "{docs}");
    assert!(
        docs.contains("through `mise install`"),
        "link only the setup steps through `mise install`: {docs}"
    );

    let development = fs::read_to_string(root.path().join("docs/development.md"))
        .expect("read docs/development.md");

    assert!(
        development.lines().any(|line| line == "## Set up"),
        "the link target #set-up has no `## Set up` heading: {development}"
    );
}

#[test]
fn herdr_docs_name_each_setup_step() {
    let root = initialized("koja");
    let docs = fs::read_to_string(root.path().join("docs/herdr.md")).expect("read docs/herdr.md");

    for step in [
        "mise install",
        "pnpm install",
        "herdr-app types mods/koja",
        "pnpm run build",
        "herdr-app check mods/koja",
        "pnpm run check",
        "curl -fsSL https://releases.herdr.dev/app/install.sh | sh",
    ] {
        assert!(docs.contains(step), "missing {step}: {docs}");
    }
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

#[test]
fn herdr_build_keeps_the_herdr_import_external() {
    let root = initialized("koja");

    install(root.path());

    let mod_path = root.path().join("mods/koja/main.ts");
    let source = fs::read_to_string(&mod_path).expect("read mods/koja/main.ts");
    let source =
        format!("{source}\nimport {{ home }} from 'herdr';\n\nexport const homeFolder = home;\n");

    fs::write(&mod_path, source).expect("append the herdr import");

    let build = run_in_project(
        root.path(),
        "pnpm",
        &["run".to_string(), "build".to_string()],
    );

    assert_eq!(build.status.code(), Some(0), "pnpm run build: {build:?}");

    let bundle =
        fs::read_to_string(root.path().join("mods/koja/main.js")).expect("read mods/koja/main.js");
    let check = run_in_project(
        root.path(),
        "herdr-app",
        &["check".to_string(), "mods/koja".to_string()],
    );

    assert!(bundle.contains("\"herdr\""), "{bundle}");
    assert_eq!(check.status.code(), Some(0), "herdr-app check: {check:?}");
}
