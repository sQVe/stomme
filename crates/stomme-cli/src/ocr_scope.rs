use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::ocr_coverage::escaped_text;

mod base;
mod change;
mod git;
mod mapping;
mod patch;

use base::{ScopedFile, build_base_commit, counts};
use change::{Change, Counts, detailed_changes, file_patch, name_status, raw_changes};
use git::{Git, TreeEntry, blob, content_lines, git_path, git_text, graft, tree_entries};
use mapping::{
    CoveredLine, RangeChange, added_lines, claimed_positions, map_covered_lines, removed_texts,
    scoped_removed_lines, uncovered_added_lines,
};
use patch::{RowKind, parse_hunks};

#[derive(Debug)]
pub(crate) struct Request {
    pub(crate) reviewed: String,
    pub(crate) reviewed_base: String,
    pub(crate) base: String,
    pub(crate) head: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Empty,
    Base {
        commit: String,
        files: usize,
        added: usize,
        removed: usize,
    },
}

#[derive(Debug, Error)]
pub(crate) enum OcrScopeError {
    #[error("the clone is shallow, so the review scope has no merge base")]
    ShallowClone,
    #[error(
        "`{flag}` names `{}`, which is not a commit in this clone",
        escaped_text(.value)
    )]
    NotACommit { flag: &'static str, value: String },
    #[error("`git {command}` failed: {}", escaped_text(.stderr))]
    Git { command: String, stderr: String },
    #[error("the path `{}` is not valid UTF-8", escaped_text(.path))]
    NotText { path: String },
}

pub(crate) fn run(root: &Path, request: &Request) -> Result<Outcome, OcrScopeError> {
    let git = Git { root };

    if is_shallow(&git)? {
        return Err(OcrScopeError::ShallowClone);
    }

    let root = repository_root(&git)?;
    let git = Git { root: &root };
    let reviewed = resolve(&git, "--reviewed", &request.reviewed)?;
    let reviewed_base = resolve(&git, "--reviewed-base", &request.reviewed_base)?;
    let base = resolve(&git, "--base", &request.base)?;
    let head = resolve(&git, "--head", &request.head)?;

    let covered = coverage(&git, &reviewed_base, &reviewed)?;
    let prior = range_changes(&git, &reviewed_base, &reviewed)?;
    let head_changes = range_changes(&git, &reviewed, &head)?;
    let head_tree = tree_entries(&git, &head)?;
    let head_lines = read_head_lines(&git, &head_tree, &head_changes)?;
    let mapped = map_covered_lines(&covered, &head_changes, &head_lines);
    let claimed = claimed_positions(&mapped);
    let removals = removals_by_new_path(&prior, &head_changes);

    let changes = detailed_changes(&git, &base, &head)?;
    let scoped = scope_changes(&git, &base, &head, &changes, &claimed, &removals)?;

    if scoped.is_empty() {
        return Ok(Outcome::Empty);
    }

    let (files, added, removed) = counts(&scoped);
    let commit = build_base_commit(&git, &head, &head_tree, &scoped)?;

    graft(&git, &head, &commit)?;

    Ok(Outcome::Base {
        commit,
        files,
        added,
        removed,
    })
}

fn is_shallow(git: &Git<'_>) -> Result<bool, OcrScopeError> {
    let arguments = ["rev-parse", "--is-shallow-repository"];
    let shallow = git_text(git, &arguments)?;

    Ok(shallow == "true")
}

fn repository_root(git: &Git<'_>) -> Result<PathBuf, OcrScopeError> {
    let arguments = ["rev-parse", "--show-toplevel"];

    git_path(git, &arguments)
}

fn resolve(git: &Git<'_>, flag: &'static str, value: &str) -> Result<String, OcrScopeError> {
    let commit = format!("{value}^{{commit}}");
    let arguments = [
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &commit,
    ];
    let output = git.run(&arguments)?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    if output.status.code() == Some(1) && output.stdout.is_empty() {
        return Err(OcrScopeError::NotACommit {
            flag,
            value: value.to_string(),
        });
    }

    match git::require_success(&arguments, &output) {
        Ok(()) => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        Err(error) => Err(error),
    }
}

fn coverage(git: &Git<'_>, base: &str, reviewed: &str) -> Result<Vec<CoveredLine>, OcrScopeError> {
    let statuses = name_status(git, base, reviewed)?;
    let mut covered = Vec::new();

    for change in statuses {
        if change.status == 'D' {
            continue;
        }

        let patch = file_patch(git, base, reviewed, &change.old_path, &change.new_path)?;
        let Some(hunks) = parse_hunks(&patch) else {
            continue;
        };

        for hunk in &hunks {
            for row in &hunk.rows {
                if row.kind != RowKind::Added {
                    continue;
                }

                let Some(line) = row.new_line else {
                    continue;
                };

                covered.push(CoveredLine {
                    path: change.new_path.clone(),
                    line,
                    text: row.text.clone(),
                });
            }
        }
    }

    Ok(covered)
}

fn range_changes(
    git: &Git<'_>,
    base: &str,
    head: &str,
) -> Result<BTreeMap<String, RangeChange>, OcrScopeError> {
    let statuses = raw_changes(git, base, head)?;
    let mut changes = BTreeMap::new();

    for change in statuses {
        if change.status == 'D' {
            continue;
        }

        // A type change keeps its entry so its covered lines stay unmappable instead of
        // claiming the old position in a file of another type.
        if is_non_regular(&change.old_mode) || is_non_regular(&change.new_mode) {
            changes.insert(
                change.old_path,
                RangeChange {
                    new_path: change.new_path,
                    hunks: None,
                },
            );
            continue;
        }

        let patch = file_patch(git, base, head, &change.old_path, &change.new_path)?;
        let hunks = parse_hunks(&patch);

        changes.insert(
            change.old_path,
            RangeChange {
                new_path: change.new_path,
                hunks,
            },
        );
    }

    Ok(changes)
}

fn read_head_lines(
    git: &Git<'_>,
    tree: &BTreeMap<String, TreeEntry>,
    changes: &BTreeMap<String, RangeChange>,
) -> Result<BTreeMap<String, Vec<Vec<u8>>>, OcrScopeError> {
    let mut lines = BTreeMap::new();

    for change in changes.values() {
        if lines.contains_key(&change.new_path) {
            continue;
        }

        let Some(entry) = tree.get(&change.new_path) else {
            continue;
        };

        if entry.kind != "blob" {
            continue;
        }

        let content = blob(git, &entry.object)?;
        let file_lines = content_lines(&content)
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect();

        lines.insert(change.new_path.clone(), file_lines);
    }

    Ok(lines)
}

// Removed texts the reviewed round already covered, keyed by the head path the reviewed path
// maps onto.
fn removals_by_new_path(
    prior: &BTreeMap<String, RangeChange>,
    changes: &BTreeMap<String, RangeChange>,
) -> BTreeMap<String, BTreeSet<Vec<u8>>> {
    let mut removals: BTreeMap<String, BTreeSet<Vec<u8>>> = BTreeMap::new();

    for change in prior.values() {
        let Some(hunks) = &change.hunks else {
            continue;
        };

        let new_path = changes
            .get(&change.new_path)
            .map_or_else(|| change.new_path.clone(), |mapped| mapped.new_path.clone());
        removals
            .entry(new_path)
            .or_default()
            .extend(removed_texts(hunks));
    }

    removals
}

fn scope_changes(
    git: &Git<'_>,
    base: &str,
    head: &str,
    changes: &[Change],
    claimed: &BTreeSet<(String, usize)>,
    removals: &BTreeMap<String, BTreeSet<Vec<u8>>>,
) -> Result<BTreeMap<String, ScopedFile>, OcrScopeError> {
    let mut scoped = BTreeMap::new();

    for change in changes {
        if is_skipped(change) {
            continue;
        }

        let patch = file_patch(git, base, head, &change.old_path, &change.new_path)?;
        let base_present = !is_zero_object(&change.old_object);

        let Some(hunks) = parse_hunks(&patch) else {
            let Counts::Text { added, removed } = change.counts else {
                continue;
            };
            let base_object = base_present.then(|| change.old_object.clone());
            scoped.insert(
                change.new_path.clone(),
                ScopedFile::Whole {
                    base_object,
                    base_mode: change.old_mode.clone(),
                    added,
                    removed,
                },
            );
            continue;
        };

        let added = uncovered_added_lines(&change.new_path, &added_lines(&hunks), claimed);
        let covered = removals.get(&change.new_path);
        let removed = scoped_removed_lines(&hunks, covered.unwrap_or(&BTreeSet::new()));

        if added.is_empty() && removed.is_empty() {
            continue;
        }

        scoped.insert(
            change.new_path.clone(),
            ScopedFile::Lines {
                added,
                removed,
                base_present,
            },
        );
    }

    Ok(scoped)
}

fn is_skipped(change: &Change) -> bool {
    change.status == 'D'
        || matches!(change.counts, Counts::Binary)
        || is_non_regular(&change.old_mode)
        || is_non_regular(&change.new_mode)
}

fn is_non_regular(mode: &str) -> bool {
    mode == "120000" || mode == "160000"
}

fn is_zero_object(object: &str) -> bool {
    !object.is_empty() && object.chars().all(|character| character == '0')
}

#[cfg(test)]
mod tests;
