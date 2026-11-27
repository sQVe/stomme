use std::fs;
use std::path::Path;

use anyhow::Context as _;

use super::{
    CaseOutcome, Ocr, Preview, RULES, admission_failures, git_text, git_text_with_index,
    init_repository, parse_preview, preview_arguments, run_ocr, temporary_folder, write_files,
};

const CASE: &str = "scope";

const BASE_KEPT: &str = "\
kept first
kept second
kept third
kept fourth
kept fifth
";

const REVIEWED_KEPT: &str = "\
kept first
kept second reviewed
kept third
kept fourth
kept fifth
";

const BASE_CHANGED: &str = "\
changed first
changed second
changed third
changed fourth
changed fifth
";

const REVIEWED_CHANGED: &str = "\
changed first
changed second reviewed
changed third
changed fourth
changed fifth
";

const HEAD_CHANGED: &str = "\
changed first
changed second reviewed
changed third
changed fourth added
changed fourth
changed fifth
";

const BASE_MAIN_ONLY: &str = "\
main first
main second
main third
main fourth
main fifth
";

const MOVED_MAIN_ONLY: &str = "\
main first
main second
main third
main fourth moved
main fifth
";

const BASE_FILES: &[(&str, &str)] = &[
    ("src/kept.rs", BASE_KEPT),
    ("src/changed.rs", BASE_CHANGED),
    ("src/main_only.rs", BASE_MAIN_ONLY),
];

const REVIEWED_FILES: &[(&str, &str)] = &[
    ("src/kept.rs", REVIEWED_KEPT),
    ("src/changed.rs", REVIEWED_CHANGED),
];

const MOVED_BASE_FILES: &[(&str, &str)] = &[
    ("src/kept.rs", BASE_KEPT),
    ("src/changed.rs", BASE_CHANGED),
    ("src/main_only.rs", MOVED_MAIN_ONLY),
];

const HEAD_FILES: &[(&str, &str)] = &[
    ("src/kept.rs", REVIEWED_KEPT),
    ("src/changed.rs", HEAD_CHANGED),
    ("src/main_only.rs", MOVED_MAIN_ONLY),
];

struct History {
    head: String,
    scoped_base: String,
}

pub(super) fn run_scope_case(ocr: &Ocr) -> anyhow::Result<CaseOutcome> {
    let folder = temporary_folder("stomme-ocr-smoke-scope-")?;
    let path = folder.path();
    init_repository(path)?;
    let history = build_history(path)?;
    check_graft(path, &history)?;
    let rules = ocr.root.join(RULES);
    let preview_output = run_ocr(
        ocr,
        &preview_arguments(path, &rules, &history.scoped_base, &history.head),
    )?;
    let preview = parse_preview(&preview_output.stdout, &preview_output.stderr)?;
    let mut admission = admission_failures(CASE, &preview, &["src/changed.rs"], &[]);

    admission.extend(absence_failures(
        &preview,
        &["src/kept.rs", "src/main_only.rs"],
    ));

    Ok(CaseOutcome {
        name: CASE,
        admission,
        precedence: Vec::new(),
    })
}

fn absence_failures(preview: &Preview, paths: &[&str]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| preview.file(path).is_some())
        .map(|path| format!("ocr-smoke: {CASE}: admission failed: {path} is in the preview"))
        .collect()
}

fn build_history(path: &Path) -> anyhow::Result<History> {
    write_files(path, BASE_FILES)?;
    git_text(path, &["add", "-A"])?;
    let base_tree = git_text(path, &["write-tree"])?;
    let base = git_text(path, &["commit-tree", &base_tree, "-m", "base"])?;

    write_files(path, REVIEWED_FILES)?;
    git_text(path, &["add", "-A"])?;
    let reviewed_tree = git_text(path, &["write-tree"])?;
    git_text(
        path,
        &["commit-tree", &reviewed_tree, "-p", &base, "-m", "reviewed"],
    )?;

    write_files(path, MOVED_BASE_FILES)?;
    git_text(path, &["add", "-A"])?;
    let moved_base_tree = git_text(path, &["write-tree"])?;
    let moved_base = git_text(
        path,
        &[
            "commit-tree",
            &moved_base_tree,
            "-p",
            &base,
            "-m",
            "moved_base",
        ],
    )?;

    write_files(path, HEAD_FILES)?;
    git_text(path, &["add", "-A"])?;
    let head_tree = git_text(path, &["write-tree"])?;
    let head = git_text(
        path,
        &["commit-tree", &head_tree, "-p", &moved_base, "-m", "head"],
    )?;

    let scoped_base = build_scoped_base(path, &head)?;
    git_text(path, &["replace", "--graft", &head, &scoped_base])?;

    Ok(History { head, scoped_base })
}

fn build_scoped_base(path: &Path, head: &str) -> anyhow::Result<String> {
    let index = path.join("scoped-base-index");
    let changed = path.join("scoped-changed.rs");
    fs::write(&changed, REVIEWED_CHANGED)
        .with_context(|| format!("Could not write {}", changed.display()))?;
    let blob = git_text(path, &["hash-object", "-w", &display_path(&changed)])?;
    git_text_with_index(path, &index, &["read-tree", head])?;
    let entry = format!("100644,{blob},src/changed.rs");
    git_text_with_index(path, &index, &["update-index", "--cacheinfo", &entry])?;
    let tree = git_text_with_index(path, &index, &["write-tree"])?;
    let commit = git_text(path, &["commit-tree", &tree, "-m", "scoped_base"])?;

    Ok(commit)
}

fn check_graft(path: &Path, history: &History) -> anyhow::Result<()> {
    let merge_base = git_text(path, &["merge-base", &history.scoped_base, &history.head])?;

    if merge_base != history.scoped_base {
        anyhow::bail!("ocr-smoke: scope: the graft did not make the base commit the head's parent");
    }

    let numstat = git_text(
        path,
        &["diff", "--numstat", &history.scoped_base, &history.head],
    )?;

    if numstat != "1\t0\tsrc/changed.rs" {
        anyhow::bail!("ocr-smoke: scope: the base commit diff is not the scope");
    }

    Ok(())
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests;
