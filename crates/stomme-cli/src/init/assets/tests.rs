use super::Source;
use crate::config;
use crate::layer::{self, Language};

use super::super::{TEST_COMMIT, planned, request};

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
        ".github/workflows/ocr.yml",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }
}

#[test]
fn plan_pins_the_sync_workflow_to_the_release_commit() {
    let plan = common_plan();
    let workflow = &plan.files[".github/workflows/sync-update.yml"];
    let pin = format!("sync-update.yml@{TEST_COMMIT} # v{VERSION}");

    assert!(workflow.contains(&pin), "{workflow}");
    assert!(!workflow.contains("sync-update.yml@v"));
    assert!(!workflow.contains("{{commit}}"));
    assert!(!workflow.contains("{{version}}"));
}

#[test]
fn plan_pins_the_ocr_workflow_to_the_release_commit() {
    let plan = common_plan();
    let workflow = &plan.files[".github/workflows/ocr.yml"];
    let pin = format!("ocr-review.yml@{TEST_COMMIT} # v{VERSION}");

    assert!(workflow.contains(&pin), "{workflow}");
    assert!(!workflow.contains("ocr-review.yml@v"));
    assert!(!workflow.contains("{{commit}}"));
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

fn numbered_steps(section: &str) -> Vec<&str> {
    let steps = section
        .lines()
        .take_while(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let (number, step) = line.split_once(". ")?;
            number.parse::<u8>().ok()?;
            Some(step)
        });

    steps.collect()
}

fn full_check_steps(development: &str) -> Vec<&str> {
    let section = development
        .split("## Run the full check")
        .nth(1)
        .expect("the development guide describes the full check");

    numbered_steps(section)
}

fn setup_steps(development: &str) -> Vec<&str> {
    let section = development
        .split("## Set up")
        .nth(1)
        .expect("the development guide describes the set up");

    numbered_steps(section)
}

fn step_running(steps: &[&str], command: &str) -> Option<usize> {
    let instruction = format!("Run `{command}`");

    steps.iter().position(|step| step.starts_with(&instruction))
}

fn step_position(language: Language, steps: &[&str], command: &str) -> usize {
    let missing = || {
        panic!(
            "the {language} starter guide docs/development.md has no step that starts with Run `{command}`"
        )
    };

    step_running(steps, command).unwrap_or_else(missing)
}

fn next_language(language: Language) -> Option<Language> {
    match language {
        Language::Rust => Some(Language::TypeScript),
        Language::TypeScript => None,
    }
}

fn supported_languages() -> Vec<Language> {
    std::iter::successors(Some(Language::Rust), |language| next_language(*language)).collect()
}

fn development_guide(language: Language) -> String {
    let request = request(language, &[], &[]);
    let sources = super::sources(&request, layer::catalog()).expect("the sources resolve");
    let refs: Vec<&Source> = sources.iter().collect();
    let plan = planned(&request, &refs);

    plan.files["docs/development.md"].clone()
}

#[test]
fn language_starters_trust_mise_before_installing() {
    for language in supported_languages() {
        let guide = development_guide(language);
        let steps = setup_steps(&guide);
        let trust = step_position(language, &steps, "mise trust mise.toml");
        let install = step_position(language, &steps, "mise install");

        assert!(
            trust < install,
            "the {language} starter guide docs/development.md runs `mise install` before `mise trust mise.toml`"
        );
    }
}

#[test]
fn step_running_accepts_only_an_affirmative_exact_command() {
    let cases: [(&str, [&str; 2], Option<usize>); 3] = [
        (
            "negated",
            ["Do not run `mise trust mise.toml`.", "Run `mise install`."],
            None,
        ),
        (
            "bare command",
            ["Run `mise trust`.", "Run `mise install`."],
            None,
        ),
        (
            "affirmative",
            ["Run `mise trust mise.toml` first.", "Run `mise install`."],
            Some(0),
        ),
    ];

    for (name, steps, expected) in cases {
        assert_eq!(
            step_running(&steps, "mise trust mise.toml"),
            expected,
            "{name}"
        );
    }
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
    assert!(plan.files["mise.toml"].contains(&format!("\"github:sQVe/stomme\" = \"{VERSION}\"")));
}

#[test]
fn plan_writes_the_typescript_check_script() {
    let plan = typescript_plan(&[]);
    let manifest = parsed_json(&plan, "package.json");
    let check_script = manifest["scripts"]["check"].as_str().unwrap();

    assert!(
        check_script.ends_with(
            r#"pnpm run test && pnpm run --if-present "/^check:layer:/" && stomme check"#
        )
    );
}

#[test]
fn plan_writes_the_typescript_full_check_steps() {
    let plan = typescript_plan(&[]);
    let steps = full_check_steps(&plan.files["docs/development.md"]);

    assert_eq!(steps.len(), 6);
    assert!(steps[4].starts_with("`pnpm run --if-present \"/^check:layer:/\"`"));
    assert_eq!(steps[5], "`stomme check`");
}

#[test]
fn full_check_steps_stop_at_the_next_heading() {
    let development = "## Run the full check\n\n1. `first`\n\n## Later\n\n1. `second`\n";

    assert_eq!(full_check_steps(development), vec!["`first`"]);
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
    assert!(manifest.get("main").is_none());
    assert_eq!(
        manifest["scripts"]["build"],
        "bun build mods/demo/main.ts --outfile mods/demo/main.js --external gpui-kit --external gpui --external gpui-base --external gpui-component --external gpui-shell --external gpui-fps --external buffer --external path --external url --external crypto --external zlib --external console --external process --external os --external fs/promises --external net --external websocket --external herdr"
    );
    assert!(manifest["scripts"]["check"].is_string());

    let tools = &parsed(&plan, "mise.toml")["tools"];

    assert!(tools["bun"].is_str());
    assert!(tools["node"].is_str());
}

#[test]
fn plan_limits_the_herdr_app_tool_to_linux() {
    let plan = typescript_plan(&["herdr"]);
    let tool = &parsed(&plan, "mise.toml")["tools"]["http:herdr-app"];
    let systems: Vec<&str> = tool["os"]
        .as_array()
        .expect("os is an array")
        .iter()
        .map(|system| system.as_str().expect("an os entry is text"))
        .collect();

    assert_eq!(systems, ["linux"]);
}

#[test]
fn plan_keeps_the_layer_files_apart() {
    let plan = typescript_plan(&["react", "pi", "herdr"]);

    for path in [
        "src/App.tsx",
        "src/piExtension.ts",
        "src/piExtension.test.ts",
        "mods/demo/main.ts",
        "mods/demo/greeting.ts",
        "mods/demo/greeting.test.ts",
        "mods/demo/tsconfig.json",
        "mods/demo/mod.json",
        "docs/herdr.md",
    ] {
        assert!(plan.files.contains_key(path), "missing {path}");
    }

    assert!(plan.files["src/App.tsx"].contains("Hello from demo."));
    assert!(plan.files["src/main.ts"].contains("greet"));
}

#[test]
fn layer_starters_keep_their_set_up_steps_in_the_layer_doc() {
    let mut layers_with_docs = 0;

    for (name, _) in layer::catalog().guides() {
        let Some(starter) = layer::catalog().starter(name) else {
            continue;
        };

        for (path, _) in starter.files {
            if path.starts_with("docs/") {
                layers_with_docs += 1;
                assert_eq!(*path, format!("docs/{name}.md"), "layer {name}");
            }
        }
    }

    assert!(layers_with_docs > 0, "no layer starter has a doc");
}

#[test]
fn typescript_development_guide_links_the_layer_docs() {
    let plan = typescript_plan(&[]);

    assert!(plan.files["docs/development.md"].contains("`docs/<layer>.md`"));
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

#[test]
fn selected_layer_source_keeps_the_catalogue_name_as_owner() {
    let request = request(Language::TypeScript, &["react"], &[]);

    let sources = super::sources(&request, layer::catalog()).expect("the sources resolve");

    assert_eq!(
        sources.last().expect("the layer adds a source").owner,
        "react"
    );
}
