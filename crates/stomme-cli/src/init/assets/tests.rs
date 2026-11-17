use super::Source;
use crate::config;
use crate::layer::{self, Language};

use super::super::{planned, request};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn common_plan() -> super::super::plan::InitPlan {
    let request = request(Language::Rust, &[], &[]);
    let sources = super::sources(&request, layer::catalog()).expect("the sources resolve");

    planned(&request, &[&sources[0]])
}

#[test]
fn plan_writes_the_common_files() {
    let plan = common_plan();

    for path in [
        "LICENSE",
        "AGENTS.md",
        "docs/README.md",
        "docs/adr/README.md",
        "docs/adr/TEMPLATE.md",
        "renovate.json",
        ".github/workflows/sync-update.yml",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }
}

#[test]
fn plan_pins_the_sync_workflow_to_the_crate_version() {
    let plan = common_plan();
    let workflow = &plan.files[".github/workflows/sync-update.yml"];

    assert!(workflow.contains(&format!("sync-update.yml@v{VERSION}")));
    assert!(!workflow.contains("{{version}}"));
}

#[test]
fn plan_writes_every_common_required_part() {
    let plan = common_plan();

    for path in [
        "docs/README.md",
        "docs/adr/README.md",
        "docs/adr/TEMPLATE.md",
        "AGENTS.md",
        "renovate.json",
        "LICENSE",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }

    for path in ["docs/development.md", ".pi/tau.json"] {
        assert!(!plan.files.contains_key(path), "common owns {path}");
    }
}

fn full_plan(layers: &[&str], roles: &[&str]) -> super::super::plan::InitPlan {
    let request = request(Language::Rust, layers, roles);
    let sources = super::sources(&request, layer::catalog()).expect("the sources resolve");
    let refs: Vec<&Source> = sources.iter().collect();

    planned(&request, &refs)
}

fn parsed(plan: &super::super::plan::InitPlan, path: &str) -> toml::Table {
    plan.files[path]
        .parse::<toml::Table>()
        .unwrap_or_else(|error| panic!("{path} is not TOML: {error}"))
}

#[test]
fn plan_writes_the_rust_base() {
    let plan = full_plan(&[], &[]);

    for path in [
        "Cargo.toml",
        "xtask/Cargo.toml",
        "xtask/src/main.rs",
        "crates/demo/Cargo.toml",
        "crates/demo/src/lib.rs",
        "rust-toolchain.toml",
        "rustfmt.toml",
        "clippy.toml",
        ".pi/tau.json",
        ".github/workflows/ci.yml",
        "mise.toml",
        "docs/development.md",
        ".gitignore",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }

    let manifest = parsed(&plan, "Cargo.toml");
    let members = manifest["workspace"]["members"].as_array().unwrap();

    assert_eq!(members, &["crates/*", "xtask"].map(toml::Value::from));
    assert_eq!(manifest["workspace"]["resolver"].as_str(), Some("3"));
    assert!(plan.files["mise.toml"].contains(&format!("\"github:sQVe/stomme\" = \"{VERSION}\"")));
}

#[test]
fn plan_writes_one_crate_per_role() {
    let roles = ["core", "ui", "kit", "app", "cli"];
    let plan = full_plan(&[], &roles);
    let configuration = parsed(&plan, config::CONFIG_FILE);

    for role in roles {
        let path = format!("crates/demo-{role}/Cargo.toml");
        let manifest = parsed(&plan, &path);

        assert_eq!(
            manifest["lints"]["workspace"].as_bool(),
            Some(true),
            "{path}"
        );
        assert_eq!(
            manifest["package"]["name"].as_str(),
            Some(format!("demo-{role}").as_str())
        );
        assert!(
            plan.files
                .contains_key(&format!("crates/demo-{role}/src/lib.rs"))
        );
        assert_eq!(
            configuration["roles"][role].as_str(),
            Some(format!("crates/demo-{role}").as_str())
        );
    }

    assert_eq!(configuration["roles"].as_table().unwrap().len(), 5);
}

#[test]
fn plan_merges_the_layer_bans() {
    let base = full_plan(&[], &[]);
    let with_gpui = full_plan(&["gpui"], &[]);
    let with_component = full_plan(&["gpui", "gpui-component"], &[]);
    let bans = parsed(&with_gpui, "clippy.toml");

    assert_eq!(bans["disallowed-methods"].as_array().unwrap().len(), 0);
    assert_eq!(bans, parsed(&base, "clippy.toml"));
    assert_eq!(
        with_gpui.files.keys().collect::<Vec<_>>(),
        with_component.files.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        with_gpui.files["clippy.toml"],
        with_component.files["clippy.toml"]
    );
}

#[test]
fn plan_copies_the_shared_lints_unchanged() {
    let plan = full_plan(&[], &[]);
    let manifest = parsed(&plan, "Cargo.toml");
    let shared: toml::Table = include_str!("../../../../../foundation/languages/rust/lints.toml")
        .parse()
        .unwrap();

    assert_eq!(manifest["workspace"]["lints"], shared["workspace"]["lints"]);
}

fn typescript_plan(layers: &[&str]) -> super::super::plan::InitPlan {
    let request = request(Language::TypeScript, layers, &[]);
    let sources = super::sources(&request, layer::catalog()).expect("the sources resolve");
    let refs: Vec<&Source> = sources.iter().collect();

    planned(&request, &refs)
}

fn parsed_json(plan: &super::super::plan::InitPlan, path: &str) -> serde_json::Value {
    serde_json::from_str(&plan.files[path])
        .unwrap_or_else(|error| panic!("{path} is not JSON: {error}"))
}

#[test]
fn plan_writes_the_typescript_base() {
    let plan = typescript_plan(&[]);

    for path in [
        "package.json",
        "tsconfig.json",
        "vite.config.ts",
        "mise.toml",
        "src/main.ts",
        "src/main.test.ts",
        ".pi/tau.json",
        ".github/workflows/ci.yml",
        "docs/development.md",
        ".gitignore",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }

    let manifest = parsed_json(&plan, "package.json");

    assert_eq!(manifest["name"], "demo");
    assert!(plan.files["tsconfig.json"].contains("mods/**/*.ts"));
    assert_eq!(manifest["scripts"]["style:check"], "stomme-ts-style");
    assert_eq!(manifest["scripts"]["typecheck"], "tsc --noEmit");
    assert!(
        manifest["scripts"]["check"]
            .as_str()
            .unwrap()
            .ends_with("&& stomme check")
    );
    assert!(plan.files["mise.toml"].contains(&format!("\"github:sQVe/stomme\" = \"{VERSION}\"")));
}

#[test]
fn plan_merges_the_layer_package_fields() {
    let plan = typescript_plan(&["react", "pi", "herdr"]);
    let manifest = parsed_json(&plan, "package.json");

    for package in ["react", "react-dom", "@types/react", "@types/react-dom"] {
        assert!(
            manifest["devDependencies"][package].is_string(),
            "missing {package}"
        );
    }

    assert_eq!(manifest["pi"]["extensions"][0], "./src/piExtension.ts");
    assert_eq!(manifest["main"], "main.js");
    assert_eq!(
        manifest["scripts"]["build"],
        "bun build mods/demo/main.ts --outfile mods/demo/main.js"
    );
    assert!(manifest["scripts"]["check"].is_string());

    let tools = &parsed(&plan, "mise.toml")["tools"];

    assert!(tools["bun"].is_str());
    assert!(tools["node"].is_str());
}

#[test]
fn plan_keeps_the_layer_files_apart() {
    let plan = typescript_plan(&["react", "pi", "herdr"]);

    for path in [
        "src/App.tsx",
        "src/piExtension.ts",
        "src/piExtension.test.ts",
        "mods/demo/main.ts",
        "mods/demo/main.test.ts",
        "mods/demo/mod.json",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }

    assert!(plan.files["src/App.tsx"].contains("Hello from demo."));
    assert!(plan.files["src/main.ts"].contains("greet"));
}

#[test]
fn plan_includes_the_catalogue_starter_for_a_selected_layer() {
    let request = request(Language::TypeScript, &["react"], &[]);
    let sources = super::sources(&request, layer::catalog()).expect("the sources resolve");
    let refs: Vec<&Source> = sources.iter().collect();

    let plan = planned(&request, &refs);

    assert!(plan.files.contains_key("src/App.tsx"));
    let manifest = parsed_json(&plan, "package.json");
    assert!(manifest["devDependencies"]["react"].is_string());
}
