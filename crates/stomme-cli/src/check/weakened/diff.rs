use std::path::Path;

use super::super::source_files;
use super::super::target_branch;
use crate::layer::Language;

pub(super) struct ChangedPath {
    pub(super) text: String,
    pub(super) language: Language,
}

pub(super) struct AddedLine {
    pub(super) line: usize,
    pub(super) text: String,
}

pub(super) fn project_prefix(root: &Path) -> Result<String, String> {
    let output = run_git(root, &["rev-parse", "--show-prefix"])?;

    Ok(output.trim_end_matches('\n').to_string())
}

pub(super) fn changed_paths(root: &Path, commit: &str) -> Result<Vec<ChangedPath>, String> {
    let output = run_git(
        root,
        &[
            "diff",
            "--name-status",
            "-z",
            "--no-renames",
            "--relative",
            commit,
        ],
    )?;
    let mut parts = output.split('\0');
    let mut paths = Vec::new();

    while let Some(_status) = parts.next() {
        let Some(path) = parts.next() else {
            break;
        };
        let Some(language) = source_language(path) else {
            continue;
        };

        paths.push(ChangedPath {
            text: path.to_string(),
            language,
        });
    }

    Ok(paths)
}

fn source_language(path: &str) -> Option<Language> {
    let (folders, name) = path.rsplit_once('/').unwrap_or(("", path));

    if folders.split('/').any(source_files::skips_folder) {
        return None;
    }

    source_files::language_of(name)
}

pub(super) fn base_text(
    root: &Path,
    commit: &str,
    prefix: &str,
    path: &str,
) -> Result<Option<String>, String> {
    let object = format!("{commit}:{prefix}{path}");
    let present = target_branch::run_git(root, &["cat-file", "-e", &object])
        .map_err(|error| format!("could not run `git`: {error}"))?;

    if !present.status.success() {
        return Ok(None);
    }

    let shown = target_branch::run_git(root, &["show", &object])
        .map_err(|error| format!("could not run `git`: {error}"))?;

    if !shown.status.success() {
        return Err(format!("cannot read `{path}` from `{commit}`"));
    }

    Ok(Some(String::from_utf8_lossy(&shown.stdout).into_owned()))
}

pub(super) fn added_lines(root: &Path, commit: &str, path: &str) -> Result<Vec<AddedLine>, String> {
    let diff = run_git(
        root,
        &[
            "--literal-pathspecs",
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "--text",
            "--no-renames",
            "--relative",
            commit,
            "--",
            path,
        ],
    )?;

    Ok(parse_added(&diff))
}

fn run_git(root: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = target_branch::run_git(root, arguments)
        .map_err(|error| format!("could not run `git`: {error}"))?;

    if !output.status.success() {
        return Err(format!("`git {}` failed", arguments.join(" ")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_added(diff: &str) -> Vec<AddedLine> {
    let mut lines = Vec::new();
    let mut next = None;

    for raw in diff.lines() {
        if let Some(start) = hunk_start(raw) {
            next = Some(start);
            continue;
        }

        let Some(line) = next.as_mut() else {
            continue;
        };

        if let Some(text) = raw.strip_prefix('+') {
            lines.push(AddedLine {
                line: *line,
                text: text.to_string(),
            });
            *line += 1;
        } else if raw.starts_with(' ') {
            *line += 1;
        }
    }

    lines
}

fn hunk_start(line: &str) -> Option<usize> {
    let rest = line.strip_prefix("@@ ")?;
    let (_, after) = rest.split_once(" +")?;
    let (numbers, _) = after.split_once(" @@")?;
    let start = numbers.split(',').next()?;

    start.parse().ok()
}
