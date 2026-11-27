use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use thiserror::Error;

use crate::ocr_coverage::escaped_text;

mod mapping;
mod patch;

use mapping::{
    CoveredLine, HeadToHeadFile, InsertedLine, Removal, added_lines, claimed_positions,
    map_covered_lines, scoped_removed_lines, uncovered_added_lines,
};
use patch::parse_hunks;

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

    let reviewed = resolve(&git, "--reviewed", &request.reviewed)?;
    let reviewed_base = resolve(&git, "--reviewed-base", &request.reviewed_base)?;
    let base = resolve(&git, "--base", &request.base)?;
    let head = resolve(&git, "--head", &request.head)?;

    let covered = coverage(&git, &reviewed_base, &reviewed)?;
    let head_changes = head_to_head_changes(&git, &reviewed, &head)?;
    let head_lines = read_head_lines(&git, &head, &head_changes)?;
    let mapped = map_covered_lines(&covered, &head_changes, &head_lines);
    let claimed = claimed_positions(&mapped);
    let removals = removals_by_head_path(&head_changes);

    let changes = detailed_changes(&git, &base, &head)?;
    let scoped = scope_changes(&git, &base, &head, &changes, &claimed, &removals)?;

    if scoped.is_empty() {
        return Ok(Outcome::Empty);
    }

    let (files, added, removed) = counts(&scoped);
    let commit = build_base_commit(&git, &head, &scoped)?;

    graft(&git, &head, &commit)?;

    Ok(Outcome::Base {
        commit,
        files,
        added,
        removed,
    })
}

struct Git<'a> {
    root: &'a Path,
}

impl Git<'_> {
    fn run(&self, arguments: &[&str]) -> Result<Output, OcrScopeError> {
        self.spawn(arguments, None, &[])
    }

    fn run_with_input(&self, arguments: &[&str], input: &[u8]) -> Result<Output, OcrScopeError> {
        self.spawn(arguments, Some(input), &[])
    }

    fn run_with_environment(
        &self,
        arguments: &[&str],
        environment: &[(&str, &str)],
    ) -> Result<Output, OcrScopeError> {
        self.spawn(arguments, None, environment)
    }

    fn spawn(
        &self,
        arguments: &[&str],
        input: Option<&[u8]>,
        environment: &[(&str, &str)],
    ) -> Result<Output, OcrScopeError> {
        let mut command = Command::new("git");

        command
            .args(["-c", "core.quotePath=false", "--no-pager"])
            .args(arguments)
            .current_dir(self.root)
            .envs(environment.iter().copied())
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| spawn_error(arguments, error))?;

        if let Some(input) = input {
            let mut stdin = child.stdin.take().expect("stdin is piped for an input");
            stdin
                .write_all(input)
                .map_err(|error| spawn_error(arguments, error))?;
        }

        child
            .wait_with_output()
            .map_err(|error| spawn_error(arguments, error))
    }
}

fn spawn_error(arguments: &[&str], error: io::Error) -> OcrScopeError {
    OcrScopeError::Git {
        command: arguments.join(" "),
        stderr: error.to_string(),
    }
}

fn malformed(command: &str) -> OcrScopeError {
    OcrScopeError::Git {
        command: command.to_string(),
        stderr: "the command printed unexpected output".to_string(),
    }
}

fn require_success(arguments: &[&str], output: &Output) -> Result<(), OcrScopeError> {
    if output.status.success() {
        return Ok(());
    }

    Err(OcrScopeError::Git {
        command: arguments.join(" "),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn git_text(git: &Git<'_>, arguments: &[&str]) -> Result<String, OcrScopeError> {
    let output = git.run(arguments)?;

    require_success(arguments, &output)?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_text_with_environment(
    git: &Git<'_>,
    arguments: &[&str],
    environment: &[(&str, &str)],
) -> Result<String, OcrScopeError> {
    let output = git.run_with_environment(arguments, environment)?;

    require_success(arguments, &output)?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn is_shallow(git: &Git<'_>) -> Result<bool, OcrScopeError> {
    let arguments = ["rev-parse", "--is-shallow-repository"];
    let shallow = git_text(git, &arguments)?;

    Ok(shallow == "true")
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

    require_success(&arguments, &output)?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct NameStatus {
    status: char,
    old_path: String,
    new_path: String,
}

fn name_status(git: &Git<'_>, base: &str, head: &str) -> Result<Vec<NameStatus>, OcrScopeError> {
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

fn path_text(bytes: &[u8]) -> Result<String, OcrScopeError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| OcrScopeError::NotText {
        path: String::from_utf8_lossy(bytes).into_owned(),
    })
}

struct RawChange {
    old_mode: String,
    new_mode: String,
    old_object: String,
    new_object: String,
}

fn raw_changes(
    git: &Git<'_>,
    base: &str,
    head: &str,
) -> Result<BTreeMap<String, RawChange>, OcrScopeError> {
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

fn parse_raw_changes(bytes: &[u8]) -> Result<BTreeMap<String, RawChange>, OcrScopeError> {
    let fields: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect();
    let mut changes = BTreeMap::new();
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
        let new_object = parts.next().ok_or_else(|| malformed("diff --raw"))?;
        let status = parts
            .next()
            .and_then(|status| status.chars().next())
            .ok_or_else(|| malformed("diff --raw"))?;
        let old_path = path_text(fields[index + 1])?;

        if is_rename(status) {
            path_text(fields[index + 2])?;
            changes.insert(
                old_path,
                RawChange {
                    old_mode: old_mode.to_string(),
                    new_mode: new_mode.to_string(),
                    old_object: old_object.to_string(),
                    new_object: new_object.to_string(),
                },
            );
            index += 3;
        } else {
            changes.insert(
                old_path,
                RawChange {
                    old_mode: old_mode.to_string(),
                    new_mode: new_mode.to_string(),
                    old_object: old_object.to_string(),
                    new_object: new_object.to_string(),
                },
            );
            index += 2;
        }
    }

    Ok(changes)
}

#[derive(Clone, Copy)]
struct Numstat {
    added: Option<usize>,
    removed: Option<usize>,
    binary: bool,
}

fn numstat(
    git: &Git<'_>,
    base: &str,
    head: &str,
) -> Result<BTreeMap<String, Numstat>, OcrScopeError> {
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

fn parse_numstat(bytes: &[u8]) -> Result<BTreeMap<String, Numstat>, OcrScopeError> {
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

fn numstat_record(added: &str, removed: &str) -> Numstat {
    if added == "-" {
        return Numstat {
            added: None,
            removed: None,
            binary: true,
        };
    }

    Numstat {
        added: added.parse().ok(),
        removed: removed.parse().ok(),
        binary: false,
    }
}

struct Change {
    status: char,
    old_path: String,
    new_path: String,
    old_mode: String,
    new_mode: String,
    old_object: String,
    new_object: String,
    added: usize,
    removed: usize,
    binary: bool,
}

fn detailed_changes(git: &Git<'_>, base: &str, head: &str) -> Result<Vec<Change>, OcrScopeError> {
    let statuses = name_status(git, base, head)?;
    let raw = raw_changes(git, base, head)?;
    let stats = numstat(git, base, head)?;
    let mut changes = Vec::new();

    for status in statuses {
        let raw = raw
            .get(&status.old_path)
            .ok_or_else(|| malformed("diff --raw"))?;
        let stat = stats.get(&status.new_path).copied().unwrap_or(Numstat {
            added: Some(0),
            removed: Some(0),
            binary: false,
        });

        changes.push(Change {
            status: status.status,
            old_path: status.old_path,
            new_path: status.new_path,
            old_mode: raw.old_mode.clone(),
            new_mode: raw.new_mode.clone(),
            old_object: raw.old_object.clone(),
            new_object: raw.new_object.clone(),
            added: stat.added.unwrap_or(0),
            removed: stat.removed.unwrap_or(0),
            binary: stat.binary,
        });
    }

    Ok(changes)
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
                let Some(line) = row.new_line else {
                    continue;
                };

                if row.old_line.is_some() {
                    continue;
                }

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

fn head_to_head_changes(
    git: &Git<'_>,
    reviewed: &str,
    head: &str,
) -> Result<BTreeMap<String, HeadToHeadFile>, OcrScopeError> {
    let statuses = name_status(git, reviewed, head)?;
    let mut changes = BTreeMap::new();

    for change in statuses {
        if change.status == 'D' {
            continue;
        }

        let patch = file_patch(git, reviewed, head, &change.old_path, &change.new_path)?;
        let hunks = parse_hunks(&patch);

        changes.insert(
            change.old_path,
            HeadToHeadFile {
                head_path: change.new_path,
                hunks,
            },
        );
    }

    Ok(changes)
}

fn file_patch(
    git: &Git<'_>,
    base: &str,
    head: &str,
    old_path: &str,
    new_path: &str,
) -> Result<String, OcrScopeError> {
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

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn read_head_lines(
    git: &Git<'_>,
    head: &str,
    changes: &BTreeMap<String, HeadToHeadFile>,
) -> Result<BTreeMap<String, Vec<Vec<u8>>>, OcrScopeError> {
    let tree = tree_entries(git, head)?;
    let mut lines = BTreeMap::new();

    for change in changes.values() {
        if lines.contains_key(&change.head_path) {
            continue;
        }

        let Some(entry) = tree.get(&change.head_path) else {
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

        lines.insert(change.head_path.clone(), file_lines);
    }

    Ok(lines)
}

fn removals_by_head_path(changes: &BTreeMap<String, HeadToHeadFile>) -> BTreeMap<String, Removal> {
    let mut removals: BTreeMap<String, Removal> = BTreeMap::new();

    for change in changes.values() {
        let removal = match &change.hunks {
            Some(hunks) => Removal::from_hunks(hunks),
            None => Removal::Every,
        };
        let existing = removals.get(&change.head_path);
        let merged = match existing {
            Some(existing) => merge_removals(existing, &removal),
            None => removal,
        };

        removals.insert(change.head_path.clone(), merged);
    }

    removals
}

fn merge_removals(first: &Removal, second: &Removal) -> Removal {
    match (first, second) {
        (Removal::Every, _) | (_, Removal::Every) => Removal::Every,
        (Removal::Texts(left), Removal::Texts(right)) => {
            let mut texts = left.clone();
            texts.extend(right.iter().cloned());
            Removal::Texts(texts)
        }
    }
}

enum ScopedFile {
    Lines {
        added: BTreeSet<usize>,
        removed: Vec<InsertedLine>,
        base_present: bool,
    },
    Whole {
        base_object: Option<String>,
        base_mode: String,
        added: usize,
        removed: usize,
    },
}

fn scope_changes(
    git: &Git<'_>,
    base: &str,
    head: &str,
    changes: &[Change],
    claimed: &BTreeSet<(String, usize)>,
    removals: &BTreeMap<String, Removal>,
) -> Result<BTreeMap<String, ScopedFile>, OcrScopeError> {
    let mut scoped = BTreeMap::new();

    for change in changes {
        if is_skipped(change) {
            continue;
        }

        let patch = file_patch(git, base, head, &change.old_path, &change.new_path)?;
        let base_present = !is_zero_object(&change.old_object);

        let Some(hunks) = parse_hunks(&patch) else {
            let base_object = base_present.then(|| change.old_object.clone());
            scoped.insert(
                change.new_path.clone(),
                ScopedFile::Whole {
                    base_object,
                    base_mode: change.old_mode.clone(),
                    added: change.added,
                    removed: change.removed,
                },
            );
            continue;
        };

        let added = uncovered_added_lines(&change.new_path, &added_lines(&hunks), claimed);
        let removed = match removals.get(&change.new_path) {
            Some(removal) => scoped_removed_lines(&hunks, removal),
            None => Vec::new(),
        };

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
    if change.status == 'D' || change.binary || is_non_regular(&change.new_mode) {
        return true;
    }

    is_mode_only(change)
}

fn is_non_regular(mode: &str) -> bool {
    mode == "120000" || mode == "160000"
}

fn is_mode_only(change: &Change) -> bool {
    change.status == 'M'
        && change.old_object == change.new_object
        && change.old_mode != change.new_mode
}

fn is_zero_object(object: &str) -> bool {
    !object.is_empty() && object.chars().all(|character| character == '0')
}

fn counts(scoped: &BTreeMap<String, ScopedFile>) -> (usize, usize, usize) {
    let mut added = 0;
    let mut removed = 0;

    for file in scoped.values() {
        match file {
            ScopedFile::Lines {
                added: lines,
                removed: lines_removed,
                ..
            } => {
                added += lines.len();
                removed += lines_removed.len();
            }
            ScopedFile::Whole {
                added: whole_added,
                removed: whole_removed,
                ..
            } => {
                added += whole_added;
                removed += whole_removed;
            }
        }
    }

    (scoped.len(), added, removed)
}

fn build_base_commit(
    git: &Git<'_>,
    head: &str,
    scoped: &BTreeMap<String, ScopedFile>,
) -> Result<String, OcrScopeError> {
    let tree = tree_entries(git, head)?;
    let mut writes = Vec::new();
    let mut drops = Vec::new();

    for (path, file) in scoped {
        match file {
            ScopedFile::Whole {
                base_object: Some(object),
                base_mode,
                ..
            } => {
                let content = blob(git, object)?;
                let object = hash_object(git, &content)?;
                writes.push((path.clone(), base_mode.clone(), object));
            }
            ScopedFile::Whole {
                base_object: None, ..
            } => drops.push(path.clone()),
            ScopedFile::Lines {
                added,
                removed,
                base_present,
            } => {
                let entry = tree.get(path).ok_or_else(|| malformed("ls-tree"))?;
                let content = blob(git, &entry.object)?;
                let rebuilt = rebuild(&content, added, removed);
                let has_lines = !content_lines(&rebuilt).is_empty();

                if !has_lines && !base_present {
                    drops.push(path.clone());
                    continue;
                }

                let object = hash_object(git, &rebuilt)?;
                writes.push((path.clone(), entry.mode.clone(), object));
            }
        }
    }

    let tree = write_tree(git, head, &writes, &drops)?;

    commit_tree(git, &tree)
}

fn rebuild(content: &[u8], added: &BTreeSet<usize>, removed: &[InsertedLine]) -> Vec<u8> {
    let lines = content_lines(content);
    let final_newline = content.ends_with(b"\n");
    let line_count = lines.len();
    let mut insertions: BTreeMap<usize, Vec<&[u8]>> = BTreeMap::new();

    for line in removed {
        insertions
            .entry(line.after)
            .or_default()
            .push(line.text.as_bytes());
    }

    let mut output: Vec<&[u8]> = Vec::new();

    push_insertions(&mut output, &insertions, 0);

    for line in 1..=line_count {
        if !added.contains(&line) {
            output.push(lines[line - 1]);
        }

        push_insertions(&mut output, &insertions, line);
    }

    let mut bytes = Vec::new();

    for (index, line) in output.iter().enumerate() {
        if index > 0 {
            bytes.push(b'\n');
        }

        bytes.extend_from_slice(line);
    }

    if final_newline && !output.is_empty() {
        bytes.push(b'\n');
    }

    bytes
}

fn push_insertions<'a>(
    output: &mut Vec<&'a [u8]>,
    insertions: &'a BTreeMap<usize, Vec<&'a [u8]>>,
    after: usize,
) {
    if let Some(texts) = insertions.get(&after) {
        output.extend(texts.iter().copied());
    }
}

fn content_lines(content: &[u8]) -> Vec<&[u8]> {
    if content.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<&[u8]> = content.split(|byte| *byte == b'\n').collect();

    if content.ends_with(b"\n") {
        lines.pop();
    }

    lines
}

struct TreeEntry {
    mode: String,
    kind: String,
    object: String,
}

fn tree_entries(
    git: &Git<'_>,
    revision: &str,
) -> Result<BTreeMap<String, TreeEntry>, OcrScopeError> {
    let arguments = ["ls-tree", "-r", "-z", revision];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)?;

    parse_tree(&output.stdout)
}

fn parse_tree(bytes: &[u8]) -> Result<BTreeMap<String, TreeEntry>, OcrScopeError> {
    let fields: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect();
    let mut entries = BTreeMap::new();

    for field in fields {
        let tab = field
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| malformed("ls-tree"))?;
        let metadata = String::from_utf8_lossy(&field[..tab]);
        let mut parts = metadata.split(' ');
        let mode = parts.next().ok_or_else(|| malformed("ls-tree"))?;
        let kind = parts.next().ok_or_else(|| malformed("ls-tree"))?;
        let object = parts.next().ok_or_else(|| malformed("ls-tree"))?;
        let path = path_text(&field[tab + 1..])?;

        entries.insert(
            path,
            TreeEntry {
                mode: mode.to_string(),
                kind: kind.to_string(),
                object: object.to_string(),
            },
        );
    }

    Ok(entries)
}

fn blob(git: &Git<'_>, object: &str) -> Result<Vec<u8>, OcrScopeError> {
    let arguments = ["cat-file", "blob", object];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)?;

    Ok(output.stdout)
}

fn hash_object(git: &Git<'_>, content: &[u8]) -> Result<String, OcrScopeError> {
    let arguments = ["hash-object", "-w", "--stdin"];
    let output = git.run_with_input(&arguments, content)?;

    require_success(&arguments, &output)?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn write_tree(
    git: &Git<'_>,
    head: &str,
    writes: &[(String, String, String)],
    drops: &[String],
) -> Result<String, OcrScopeError> {
    let directory = git_text(git, &["rev-parse", "--absolute-git-dir"])?;
    let index_path =
        PathBuf::from(directory).join(format!("stomme-ocr-scope-index-{}", std::process::id()));
    let _guard = TemporaryIndex {
        path: index_path.clone(),
    };
    let index_text = index_path.to_str().ok_or_else(|| OcrScopeError::NotText {
        path: index_path.display().to_string(),
    })?;
    let environment = [("GIT_INDEX_FILE", index_text)];

    run_index(git, &environment, &["read-tree", head])?;

    for (path, mode, object) in writes {
        let entry = format!("{mode},{object},{path}");

        run_index(git, &environment, &["update-index", "--cacheinfo", &entry])?;
    }

    for path in drops {
        run_index(git, &environment, &["update-index", "--force-remove", path])?;
    }

    git_text_with_environment(git, &["write-tree"], &environment)
}

fn run_index(
    git: &Git<'_>,
    environment: &[(&str, &str)],
    arguments: &[&str],
) -> Result<(), OcrScopeError> {
    let output = git.run_with_environment(arguments, environment)?;

    require_success(arguments, &output)
}

struct TemporaryIndex {
    path: PathBuf,
}

impl Drop for TemporaryIndex {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            let message = format!(
                "stomme: could not remove {}: {error}\n",
                self.path.display()
            );
            let mut stderr = io::stderr().lock();

            match stderr.write_all(message.as_bytes()) {
                Ok(()) | Err(_) => {}
            }
        }
    }
}

fn commit_tree(git: &Git<'_>, tree: &str) -> Result<String, OcrScopeError> {
    let arguments = ["commit-tree", tree, "-m", "stomme ocr-scope base"];
    let environment = [
        ("GIT_AUTHOR_NAME", "stomme"),
        ("GIT_AUTHOR_EMAIL", "stomme@localhost"),
        ("GIT_COMMITTER_NAME", "stomme"),
        ("GIT_COMMITTER_EMAIL", "stomme@localhost"),
        ("GIT_AUTHOR_DATE", "@0 +0000"),
        ("GIT_COMMITTER_DATE", "@0 +0000"),
    ];

    git_text_with_environment(git, &arguments, &environment)
}

fn graft(git: &Git<'_>, head: &str, commit: &str) -> Result<(), OcrScopeError> {
    // A rerun replaces the graft of the same head, which `-f` allows.
    let arguments = ["replace", "-f", "--graft", head, commit];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)
}

#[cfg(test)]
mod tests;
