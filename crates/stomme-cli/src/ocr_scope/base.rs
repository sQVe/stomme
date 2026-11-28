use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write as _};
use std::path::PathBuf;

use super::OcrScopeError;
use super::git::{
    Git, TreeEntry, blob, commit_tree, content_lines, git_path, git_text_with_environment,
    hash_object, require_success,
};
use super::mapping::InsertedLine;

pub(super) enum ScopedFile {
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

pub(super) fn counts(scoped: &BTreeMap<String, ScopedFile>) -> (usize, usize, usize) {
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

pub(super) fn build_base_commit(
    git: &Git<'_>,
    head: &str,
    tree: &BTreeMap<String, TreeEntry>,
    scoped: &BTreeMap<String, ScopedFile>,
) -> Result<String, OcrScopeError> {
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
                let entry = tree
                    .get(path)
                    .ok_or_else(|| OcrScopeError::MissingHeadEntry { path: path.clone() })?;
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
    let mut insertions: BTreeMap<usize, Vec<&InsertedLine>> = BTreeMap::new();

    for line in removed {
        insertions.entry(line.after).or_default().push(line);
    }

    let mut output: Vec<OutputLine<'_>> = Vec::new();

    push_insertions(&mut output, &insertions, 0);

    for line in 1..=line_count {
        if !added.contains(&line) {
            let terminated = line < line_count || final_newline;
            let text = lines[line - 1];

            output.push(OutputLine::Head { text, terminated });
        }

        push_insertions(&mut output, &insertions, line);
    }

    serialize(&output)
}

enum OutputLine<'a> {
    Head { text: &'a [u8], terminated: bool },
    Restored(&'a InsertedLine),
}

fn serialize(lines: &[OutputLine<'_>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let last = lines.len().saturating_sub(1);

    for (index, line) in lines.iter().enumerate() {
        let (text, terminated) = match line {
            OutputLine::Head { text, terminated } => (*text, *terminated),
            OutputLine::Restored(restored) => (restored.text.as_slice(), restored.terminated),
        };

        bytes.extend_from_slice(text);

        if index < last || terminated {
            bytes.push(b'\n');
        }
    }

    bytes
}

fn push_insertions<'a>(
    output: &mut Vec<OutputLine<'a>>,
    insertions: &'a BTreeMap<usize, Vec<&'a InsertedLine>>,
    after: usize,
) {
    if let Some(lines) = insertions.get(&after) {
        for line in lines {
            output.push(OutputLine::Restored(line));
        }
    }
}

fn write_tree(
    git: &Git<'_>,
    head: &str,
    writes: &[(String, String, String)],
    drops: &[String],
) -> Result<String, OcrScopeError> {
    let directory = git_path(git, &["rev-parse", "--absolute-git-dir"])?;
    let index_path = directory.join(format!("stomme-ocr-scope-index-{}", std::process::id()));
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
        run_index(
            git,
            &environment,
            &["update-index", "--force-remove", "--", path],
        )?;
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

#[cfg(test)]
mod tests;
