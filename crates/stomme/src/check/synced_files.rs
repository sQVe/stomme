use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use anyhow::Context as _;

use super::Problem;
use crate::config::Selection;
use crate::owned::{self, DOCS_FOLDER};
use crate::rule::Rule;
use crate::sync::{self, RECORD_FILE, Record};

pub(super) fn check(root: &Path, selection: &Selection) -> anyhow::Result<Vec<Problem>> {
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

    let owned = owned::owned_set(selection);
    let owned_paths: BTreeSet<&str> = owned.iter().map(|file| file.path.as_str()).collect();
    let recorded_paths: BTreeSet<&str> = record.files.keys().map(String::as_str).collect();
    let known_paths: BTreeSet<&str> = owned_paths.union(&recorded_paths).copied().collect();

    for file in &owned {
        check_owned_file(root, file, &record, &mut problems)?;
    }

    for path in &recorded_paths {
        if !owned_paths.contains(path) {
            problems.push(problem(
                path,
                "is recorded but not owned by this stomme version; run `stomme sync`",
            ));
        }
    }

    check_extra_files(root, DOCS_FOLDER, &known_paths, &mut problems)?;

    Ok(problems)
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
    let entries = match fs::read_dir(root.join(folder)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {folder}"));
        }
    };

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

#[cfg(test)]
mod tests;
