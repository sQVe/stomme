use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context as _;

use super::Problem;
use crate::agents_block;
use crate::config::Selection;
use crate::full_check;
use crate::owned::{self, DOCS_FOLDER};
use crate::rule::Rule;
use crate::sync::{self, RECORD_FILE, Record};

pub(super) fn check(root: &Path, selection: &Selection) -> anyhow::Result<Vec<Problem>> {
    let owned = owned::owned_set(selection);

    if let Some(problem) = parent_problem(root, RECORD_FILE) {
        return Ok(vec![problem]);
    }

    for file in &owned {
        if let Some(problem) = parent_problem(root, &file.path) {
            return Ok(vec![problem]);
        }
    }

    let mut problems = Vec::new();
    let record = match Record::read(root) {
        Ok(Some(record)) => record,
        Ok(None) => {
            problems.push(problem(RECORD_FILE, "is missing; run `stomme sync`"));

            return Ok(problems);
        }
        Err(error) => {
            problems.push(problem(RECORD_FILE, &error.to_string()));

            return Ok(problems);
        }
    };

    if record.version != selection.version {
        let message = format!(
            "records version {}; this stomme is version {}; run `stomme sync`",
            record.version, selection.version
        );

        problems.push(problem(RECORD_FILE, &message));
    }

    let owned_paths: BTreeSet<&str> = owned.iter().map(|file| file.path.as_str()).collect();
    let recorded_paths: BTreeSet<&str> = record.files.keys().map(String::as_str).collect();
    let known_paths: BTreeSet<&str> = owned_paths.union(&recorded_paths).copied().collect();

    for file in &owned {
        check_owned_file(root, file, &record, &mut problems)?;
    }

    for path in &recorded_paths {
        if !owned_paths.contains(path) && *path != agents_block::PATH {
            problems.push(problem(
                path,
                "is recorded but not owned by this stomme version; run `stomme sync`",
            ));
        }
    }

    check_block(root, selection, &record, &mut problems)?;
    check_extra_files(root, DOCS_FOLDER, &known_paths, &mut problems)?;

    Ok(problems)
}

fn check_block(
    root: &Path,
    selection: &Selection,
    record: &Record,
    problems: &mut Vec<Problem>,
) -> anyhow::Result<()> {
    let path = root.join(agents_block::PATH);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", path.display()));
        }
    };

    if !metadata.is_file() {
        problems.push(block_problem(
            agents_block::PATH,
            "is not a regular file; run `stomme sync`",
        ));

        return Ok(());
    }

    let bytes = fs::read(&path).with_context(|| format!("Could not read {}", path.display()))?;
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            problems.push(block_problem(
                agents_block::PATH,
                "is not valid UTF-8; run `stomme sync`",
            ));

            return Ok(());
        }
    };
    let argv = full_check::read(root).ok().flatten();
    let block = agents_block::render(selection, argv.as_deref());
    let range = match agents_block::find(&text) {
        Ok(Some(range)) => range,
        Ok(None) => {
            problems.push(block_problem(
                agents_block::PATH,
                "is missing the managed block; run `stomme sync`",
            ));

            return Ok(());
        }
        Err(error) => {
            problems.push(block_problem(agents_block::PATH, &error.to_string()));

            return Ok(());
        }
    };
    let current = &text[range];
    let actual = sync::hash(current.as_bytes());
    let expected = sync::hash(block.as_bytes());

    if actual != expected {
        problems.push(block_problem(
            agents_block::PATH,
            "differs from the synced block; run `stomme sync`",
        ));

        return Ok(());
    }

    match record.files.get(agents_block::PATH) {
        Some(recorded) if *recorded == actual => {}
        Some(_) => problems.push(block_problem(
            agents_block::PATH,
            "does not match the ownership record; run `stomme sync`",
        )),
        None => problems.push(block_problem(
            agents_block::PATH,
            "is not in the ownership record; run `stomme sync`",
        )),
    }

    Ok(())
}

fn check_owned_file(
    root: &Path,
    file: &owned::OwnedFile,
    record: &Record,
    problems: &mut Vec<Problem>,
) -> anyhow::Result<()> {
    let path = root.join(&file.path);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            problems.push(problem(&file.path, "is missing; run `stomme sync`"));

            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", path.display()));
        }
    };

    if !metadata.is_file() {
        problems.push(problem(
            &file.path,
            "is not a regular file; run `stomme sync`",
        ));

        return Ok(());
    }

    let bytes = fs::read(&path).with_context(|| format!("Could not read {}", path.display()))?;
    let actual = sync::hash(&bytes);
    let expected = sync::hash(file.content.as_bytes());

    if actual != expected {
        problems.push(problem(
            &file.path,
            "differs from the synced content; run `stomme sync`",
        ));

        return Ok(());
    }

    match record.files.get(&file.path) {
        Some(recorded) if *recorded == actual => {}
        Some(_) => problems.push(problem(
            &file.path,
            "does not match the ownership record; run `stomme sync`",
        )),
        None => problems.push(problem(
            &file.path,
            "is not in the ownership record; run `stomme sync`",
        )),
    }

    Ok(())
}

fn check_extra_files(
    root: &Path,
    folder: &str,
    known: &BTreeSet<&str>,
    problems: &mut Vec<Problem>,
) -> anyhow::Result<()> {
    let entries =
        fs::read_dir(root.join(folder)).with_context(|| format!("Could not read {folder}"))?;

    for entry in entries {
        let entry = entry.with_context(|| format!("Could not read {folder}"))?;
        let path = entry.path();
        let relative = relative_path(root, &path);

        if relative == RECORD_FILE {
            continue;
        }

        let file_type = entry
            .file_type()
            .with_context(|| format!("Could not read {relative}"))?;

        if file_type.is_dir() {
            check_extra_files(root, &relative, known, problems)?;
        } else if !known.contains(relative.as_str()) {
            problems.push(problem(&relative, "is not owned by stomme; remove it"));
        }
    }

    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("a docs/stomme entry lies inside the root")
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn problem(path: &str, message: &str) -> Problem {
    Problem {
        path: path.to_string(),
        rule: Rule::SyncedFiles,
        message: message.to_string(),
    }
}

fn block_problem(path: &str, message: &str) -> Problem {
    Problem {
        path: path.to_string(),
        rule: Rule::AgentsBlock,
        message: message.to_string(),
    }
}

fn parent_problem(root: &Path, path: &str) -> Option<Problem> {
    let error = sync::check_parent_folders(root, path).err()?;
    let path = match &error {
        sync::SyncError::SymlinkedFolder { path }
        | sync::SyncError::NotAFile { path }
        | sync::SyncError::Read { path, .. } => path.clone(),
        _ => String::new(),
    };

    Some(Problem {
        path,
        rule: Rule::SyncedFiles,
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests;
