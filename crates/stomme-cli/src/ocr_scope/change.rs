use std::collections::BTreeMap;

use super::OcrScopeError;
use super::git::{Git, malformed, path_text, require_success};

pub(super) struct NameStatus {
    pub(super) status: char,
    pub(super) old_path: String,
    pub(super) new_path: String,
}

pub(super) fn name_status(
    git: &Git<'_>,
    base: &str,
    head: &str,
) -> Result<Vec<NameStatus>, OcrScopeError> {
    let arguments = [
        "diff",
        "--name-status",
        "-z",
        "--find-renames",
        "--no-ext-diff",
        "--no-color",
        "--no-textconv",
        base,
        head,
    ];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)?;

    parse_name_status(&output.stdout)
}

fn parse_name_status(bytes: &[u8]) -> Result<Vec<NameStatus>, OcrScopeError> {
    let fields: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect();
    let mut changes = Vec::new();
    let mut index = 0;

    while index < fields.len() {
        let status_text = String::from_utf8_lossy(fields[index]);
        let status = status_text
            .chars()
            .next()
            .ok_or_else(|| malformed("diff --name-status"))?;
        let old_path = path_text(fields[index + 1])?;

        if is_rename(status) {
            let new_path = path_text(fields[index + 2])?;
            changes.push(NameStatus {
                status,
                old_path,
                new_path,
            });
            index += 3;
        } else {
            changes.push(NameStatus {
                status,
                new_path: old_path.clone(),
                old_path,
            });
            index += 2;
        }
    }

    Ok(changes)
}

fn is_rename(status: char) -> bool {
    status == 'R' || status == 'C'
}

struct RawChange {
    status: char,
    old_path: String,
    new_path: String,
    old_mode: String,
    new_mode: String,
    old_object: String,
}

fn raw_changes(git: &Git<'_>, base: &str, head: &str) -> Result<Vec<RawChange>, OcrScopeError> {
    let arguments = [
        "diff",
        "--raw",
        "-z",
        "--find-renames",
        "--no-ext-diff",
        "--no-color",
        "--no-textconv",
        base,
        head,
    ];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)?;

    parse_raw_changes(&output.stdout)
}

fn parse_raw_changes(bytes: &[u8]) -> Result<Vec<RawChange>, OcrScopeError> {
    let fields: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect();
    let mut changes = Vec::new();
    let mut index = 0;

    while index < fields.len() {
        let metadata = String::from_utf8_lossy(fields[index]);
        let mut parts = metadata.split(' ');
        let old_mode = parts
            .next()
            .and_then(|mode| mode.strip_prefix(':'))
            .ok_or_else(|| malformed("diff --raw"))?;
        let new_mode = parts.next().ok_or_else(|| malformed("diff --raw"))?;
        let old_object = parts.next().ok_or_else(|| malformed("diff --raw"))?;
        let _new_object = parts.next().ok_or_else(|| malformed("diff --raw"))?;
        let status = parts
            .next()
            .and_then(|status| status.chars().next())
            .ok_or_else(|| malformed("diff --raw"))?;
        let old_path = path_text(fields[index + 1])?;

        if is_rename(status) {
            let new_path = path_text(fields[index + 2])?;
            changes.push(RawChange {
                status,
                old_path,
                new_path,
                old_mode: old_mode.to_string(),
                new_mode: new_mode.to_string(),
                old_object: old_object.to_string(),
            });
            index += 3;
        } else {
            changes.push(RawChange {
                status,
                new_path: old_path.clone(),
                old_path,
                old_mode: old_mode.to_string(),
                new_mode: new_mode.to_string(),
                old_object: old_object.to_string(),
            });
            index += 2;
        }
    }

    Ok(changes)
}

#[derive(Clone, Copy)]
pub(super) enum Counts {
    Binary,
    Text { added: usize, removed: usize },
}

fn numstat(
    git: &Git<'_>,
    base: &str,
    head: &str,
) -> Result<BTreeMap<String, Counts>, OcrScopeError> {
    let arguments = [
        "diff",
        "--numstat",
        "-z",
        "--find-renames",
        "--no-ext-diff",
        "--no-color",
        "--no-textconv",
        base,
        head,
    ];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)?;

    parse_numstat(&output.stdout)
}

fn parse_numstat(bytes: &[u8]) -> Result<BTreeMap<String, Counts>, OcrScopeError> {
    let fields: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect();
    let mut stats = BTreeMap::new();
    let mut index = 0;

    while index < fields.len() {
        let record = String::from_utf8_lossy(fields[index]);
        let mut parts = record.splitn(3, '\t');
        let added = parts.next().ok_or_else(|| malformed("diff --numstat"))?;
        let removed = parts.next().ok_or_else(|| malformed("diff --numstat"))?;
        let path = parts.next().ok_or_else(|| malformed("diff --numstat"))?;

        if path.is_empty() {
            let new_path = path_text(fields[index + 2])?;
            stats.insert(new_path, numstat_record(added, removed));
            index += 3;
        } else {
            stats.insert(path.to_string(), numstat_record(added, removed));
            index += 1;
        }
    }

    Ok(stats)
}

fn numstat_record(added: &str, removed: &str) -> Counts {
    if added == "-" {
        return Counts::Binary;
    }

    Counts::Text {
        added: added.parse().unwrap_or(0),
        removed: removed.parse().unwrap_or(0),
    }
}

pub(super) struct Change {
    pub(super) status: char,
    pub(super) old_path: String,
    pub(super) new_path: String,
    pub(super) old_mode: String,
    pub(super) new_mode: String,
    pub(super) old_object: String,
    pub(super) counts: Counts,
}

pub(super) fn detailed_changes(
    git: &Git<'_>,
    base: &str,
    head: &str,
) -> Result<Vec<Change>, OcrScopeError> {
    let raw = raw_changes(git, base, head)?;
    let stats = numstat(git, base, head)?;
    let mut changes = Vec::new();

    for change in raw {
        let counts = stats
            .get(&change.new_path)
            .copied()
            .unwrap_or(Counts::Text {
                added: 0,
                removed: 0,
            });

        changes.push(Change {
            status: change.status,
            old_path: change.old_path,
            new_path: change.new_path,
            old_mode: change.old_mode,
            new_mode: change.new_mode,
            old_object: change.old_object,
            counts,
        });
    }

    Ok(changes)
}

pub(super) fn file_patch(
    git: &Git<'_>,
    base: &str,
    head: &str,
    old_path: &str,
    new_path: &str,
) -> Result<Vec<u8>, OcrScopeError> {
    let mut arguments = vec![
        "diff",
        "-U0",
        "--find-renames",
        "--no-ext-diff",
        "--no-color",
        "--no-textconv",
        base,
        head,
        "--",
    ];

    let old_pathspec = format!(":(literal){old_path}");
    let new_pathspec = format!(":(literal){new_path}");

    arguments.push(&old_pathspec);

    if old_path != new_path {
        arguments.push(&new_pathspec);
    }

    let output = git.run(&arguments)?;

    require_success(&arguments, &output)?;

    Ok(output.stdout)
}

#[cfg(test)]
mod tests;
