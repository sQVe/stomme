use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::sync::{self, SyncError};

fn project(agents: Option<&str>) -> TempDir {
    let root = tempfile::Builder::new()
        .prefix("stomme-agents-")
        .tempdir()
        .expect("create a temporary directory");
    let config = format!(
        "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n",
        env!("CARGO_PKG_VERSION")
    );

    fs::write(root.path().join("stomme.toml"), config).expect("write stomme.toml");
    fs::create_dir_all(root.path().join(".pi")).expect("create .pi");
    fs::write(
        root.path().join(".pi/tau.json"),
        r#"{ "fullCheck": { "argv": ["cargo", "xtask", "check"] } }"#,
    )
    .expect("write .pi/tau.json");

    if let Some(agents) = agents {
        fs::write(root.path().join(PATH), agents).expect("write AGENTS.md");
    }

    root
}

fn sync_project(root: &Path) -> Result<(), SyncError> {
    sync::run(root, crate::layer::catalog())
}

fn read_agents(root: &Path) -> String {
    fs::read_to_string(root.join(PATH)).expect("read AGENTS.md")
}

#[test]
fn sync_writes_the_block_and_keeps_the_text_outside() {
    let original = "# Project\n\nOwn text.\n";
    let root = project(Some(original));

    sync_project(root.path()).expect("sync succeeds");

    let text = read_agents(root.path());
    let range = find(&text)
        .expect("find the block")
        .expect("the block exists");

    assert_eq!(&text[..range.start], original);
    assert_eq!(&text[range.end..], "");
    assert!(text.contains("`cargo xtask check`"), "{text}");
    assert!(text.contains("docs/stomme/shared/coding.md"), "{text}");
    assert!(
        text.contains("docs/stomme/languages/rust/guide.md"),
        "{text}"
    );
}

#[test]
fn sync_twice_leaves_agents_md_unchanged() {
    let root = project(Some("# Project\n"));

    sync_project(root.path()).expect("the first sync succeeds");
    let first = read_agents(root.path());

    sync_project(root.path()).expect("the second sync succeeds");

    assert_eq!(read_agents(root.path()), first);
}

#[test]
fn sync_refuses_an_edited_block() {
    let root = project(Some("# Project\n"));

    sync_project(root.path()).expect("the first sync succeeds");

    let text = read_agents(root.path());
    let range = find(&text)
        .expect("find the block")
        .expect("the block exists");
    let edited = format!(
        "{}{START}\nedited\n{END}\n{}",
        &text[..range.start],
        &text[range.end..]
    );

    fs::write(root.path().join(PATH), &edited).expect("edit the block");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Edited { ref path } if path == PATH),
        "{error:?}"
    );
    assert_eq!(read_agents(root.path()), edited);
}

#[test]
fn sync_refuses_a_missing_marker() {
    let root = project(Some("# Project\n"));

    sync_project(root.path()).expect("the first sync succeeds");

    let text = read_agents(root.path());
    let without_end = text.replace(&format!("{END}\n"), "");

    fs::write(root.path().join(PATH), &without_end).expect("remove the end marker");

    let error = sync_project(root.path()).expect_err("sync refuses");

    assert!(
        matches!(error, SyncError::Block { ref path, .. } if path == PATH),
        "{error:?}"
    );
    assert_eq!(read_agents(root.path()), without_end);
}

#[test]
fn sync_does_not_create_agents_md() {
    let root = project(None);

    sync_project(root.path()).expect("sync succeeds");

    assert!(!root.path().join(PATH).exists());
}

#[test]
fn render_quotes_arguments_with_whitespace_or_a_backtick() {
    let selection = Selection {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language: Language::Rust,
        layers: Default::default(),
        roles: Default::default(),
        extra_docs: Vec::new(),
    };
    let argv = vec![
        "cargo".to_string(),
        "run with space".to_string(),
        "tick`arg".to_string(),
    ];

    let block = render(&selection, Some(&argv));

    assert!(
        block.contains("``cargo \"run with space\" \"tick`arg\"``"),
        "{block}"
    );
}

#[test]
fn replace_appends_a_block_after_text_without_a_final_newline() {
    let block = "<!-- stomme:start -->\nbody\n<!-- stomme:end -->\n";

    let updated = replace("text", block).expect("replace the block");

    assert_eq!(updated, format!("text\n{block}"));
}

#[test]
fn find_rejects_an_end_marker_before_a_start_marker() {
    let text = format!("{END}\n{START}\n");

    assert_eq!(find(&text), Err(BlockError::Reversed));
}

#[test]
fn sync_writes_links_that_resolve() {
    let root = project(Some("# Project\n"));

    fs::create_dir_all(root.path().join("docs/adr")).expect("create docs/adr");
    fs::write(root.path().join("docs/adr/README.md"), "# ADRs\n").expect("write the ADR index");
    fs::write(
        root.path().join("docs/adr/0001-documentation-scope.md"),
        "# ADR 0001: Documentation scope\n",
    )
    .expect("write the ADR");

    sync_project(root.path()).expect("sync succeeds");

    let text = read_agents(root.path());
    let range = find(&text)
        .expect("find the block")
        .expect("the block exists");

    for target in link_targets(&text[range]) {
        assert!(
            root.path().join(&target).is_file(),
            "{target} does not resolve"
        );
    }
}

fn link_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = text;

    while let Some((_, after_open)) = rest.split_once("](") {
        let Some((target, after_close)) = after_open.split_once(')') else {
            break;
        };

        targets.push(target.to_string());
        rest = after_close;
    }

    targets
}
