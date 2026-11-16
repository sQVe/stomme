use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::config::{self, Installation, Selection};
use crate::layer::Catalog;
use crate::owned::{self, DOCS_FOLDER, LinkError, OwnedFile};

pub(crate) const RECORD_FILE: &str = "docs/stomme/.sync-record.toml";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) files: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub(crate) enum SyncError {
    #[error("stomme.toml is missing; add one with `schema = 1`, `version`, and a `language`")]
    NoStomme,
    #[error("stomme.toml {reason}")]
    Broken { reason: String },
    #[error(transparent)]
    Link(#[from] LinkError),
    #[error("the ownership record {path} is malformed: {reason}")]
    MalformedRecord { path: String, reason: String },
    #[error("{path} is not a regular file")]
    NotAFile { path: String },
    #[error("{path} differs from the synced content; edit it or remove it")]
    Edited { path: String },
    #[error("{path} is not owned by stomme; remove it")]
    Extra { path: String },
    #[error("could not read {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error("could not write {path}: {source}")]
    Write { path: String, source: io::Error },
    #[error("could not remove {path}: {source}")]
    Remove { path: String, source: io::Error },
}

type PlannedWrites = (Vec<(String, &'static str)>, BTreeMap<String, String>);

struct Plan {
    writes: Vec<(String, &'static str)>,
    deletes: Vec<String>,
    record: Record,
}

pub(crate) fn run(root: &Path, catalog: &Catalog) -> Result<(), SyncError> {
    let selection = match config::read(root, catalog) {
        Installation::Valid(selection) => selection,
        Installation::NoStomme => return Err(SyncError::NoStomme),
        Installation::Broken(reason) => return Err(SyncError::Broken { reason }),
    };
    let owned = owned::owned_set(&selection);

    sync(root, &selection, &owned)
}

pub(crate) fn hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);

    let mut text = String::with_capacity(digest.len() * 2);

    for byte in digest {
        write!(&mut text, "{byte:02x}").expect("writing to a string cannot fail");
    }

    text
}

impl Record {
    pub(crate) fn read(root: &Path) -> Result<Option<Self>, SyncError> {
        let path = root.join(RECORD_FILE);

        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(SyncError::MalformedRecord {
                    path: RECORD_FILE.to_string(),
                    reason: "is not a regular file".to_string(),
                });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(SyncError::Read {
                    path: RECORD_FILE.to_string(),
                    source,
                });
            }
        }

        let text = fs::read_to_string(&path).map_err(|source| SyncError::Read {
            path: RECORD_FILE.to_string(),
            source,
        })?;
        let record =
            toml::from_str(&text).map_err(|error: toml::de::Error| SyncError::MalformedRecord {
                path: RECORD_FILE.to_string(),
                reason: error.message().to_string(),
            })?;

        Ok(Some(record))
    }
}

fn sync(root: &Path, selection: &Selection, owned: &[OwnedFile]) -> Result<(), SyncError> {
    owned::validate_links(owned)?;

    let plan = plan(root, selection, owned)?;

    execute(root, plan)
}

fn plan(root: &Path, selection: &Selection, owned: &[OwnedFile]) -> Result<Plan, SyncError> {
    let record = Record::read(root)?;

    if let Some(record) = &record {
        validate_record(record)?;
    }

    let owned_paths: BTreeSet<&str> = owned.iter().map(|file| file.path.as_str()).collect();
    let recorded_paths: BTreeSet<&str> = record
        .as_ref()
        .map(|record| record.files.keys().map(String::as_str).collect())
        .unwrap_or_default();

    for path in owned_paths.iter().chain(&recorded_paths) {
        check_regular_file(root, path)?;
    }

    check_extra_files(root, &owned_paths, &recorded_paths)?;

    let (writes, files) = plan_writes(root, owned, record.as_ref())?;
    let deletes = plan_deletes(root, record.as_ref(), &owned_paths)?;
    let record = Record {
        version: selection.version.clone(),
        files,
    };

    Ok(Plan {
        writes,
        deletes,
        record,
    })
}

fn plan_writes(
    root: &Path,
    owned: &[OwnedFile],
    record: Option<&Record>,
) -> Result<PlannedWrites, SyncError> {
    let mut writes = Vec::new();
    let mut files = BTreeMap::new();

    for file in owned {
        files.insert(file.path.clone(), hash(file.content.as_bytes()));

        let Some(existing) = read_existing(root, &file.path)? else {
            writes.push((file.path.clone(), file.content));
            continue;
        };

        if existing == file.content.as_bytes() {
            continue;
        }

        let recorded = record
            .and_then(|record| record.files.get(&file.path))
            .is_some_and(|recorded| *recorded == hash(&existing));

        if !recorded {
            return Err(SyncError::Edited {
                path: file.path.clone(),
            });
        }

        writes.push((file.path.clone(), file.content));
    }

    Ok((writes, files))
}

fn plan_deletes(
    root: &Path,
    record: Option<&Record>,
    owned_paths: &BTreeSet<&str>,
) -> Result<Vec<String>, SyncError> {
    let Some(record) = record else {
        return Ok(Vec::new());
    };
    let mut deletes = Vec::new();

    for (path, recorded_hash) in &record.files {
        if owned_paths.contains(path.as_str()) {
            continue;
        }

        let Some(existing) = read_existing(root, path)? else {
            continue;
        };

        if hash(&existing) != *recorded_hash {
            return Err(SyncError::Edited { path: path.clone() });
        }

        deletes.push(path.clone());
    }

    Ok(deletes)
}

fn execute(root: &Path, plan: Plan) -> Result<(), SyncError> {
    for (path, content) in &plan.writes {
        write_file(root, path, content)?;
    }

    for path in &plan.deletes {
        remove_file(root, path)?;
    }

    write_record(root, &plan.record)
}

fn validate_record(record: &Record) -> Result<(), SyncError> {
    for path in record.files.keys() {
        let inside = path.starts_with(&format!("{DOCS_FOLDER}/"));
        let clean = !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");

        if path == RECORD_FILE || !inside || !clean {
            return Err(SyncError::MalformedRecord {
                path: RECORD_FILE.to_string(),
                reason: format!("names `{path}`, which is not an owned file"),
            });
        }
    }

    Ok(())
}

fn check_regular_file(root: &Path, path: &str) -> Result<(), SyncError> {
    if let Some(metadata) = metadata(root, path)?
        && !metadata.is_file()
    {
        return Err(SyncError::NotAFile {
            path: path.to_string(),
        });
    }

    Ok(())
}

fn check_extra_files(
    root: &Path,
    owned: &BTreeSet<&str>,
    recorded: &BTreeSet<&str>,
) -> Result<(), SyncError> {
    collect_extra_files(root, DOCS_FOLDER, owned, recorded)
}

fn collect_extra_files(
    root: &Path,
    folder: &str,
    owned: &BTreeSet<&str>,
    recorded: &BTreeSet<&str>,
) -> Result<(), SyncError> {
    let entries = match fs::read_dir(root.join(folder)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(SyncError::Read {
                path: folder.to_string(),
                source,
            });
        }
    };

    for entry in entries {
        let entry = entry.map_err(|source| SyncError::Read {
            path: folder.to_string(),
            source,
        })?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("a docs/stomme entry lies inside the root")
            .to_string_lossy()
            .replace('\\', "/");

        if relative == RECORD_FILE {
            continue;
        }

        let file_type = entry.file_type().map_err(|source| SyncError::Read {
            path: relative.clone(),
            source,
        })?;

        if file_type.is_dir() {
            collect_extra_files(root, &relative, owned, recorded)?;
        } else if !owned.contains(relative.as_str()) && !recorded.contains(relative.as_str()) {
            return Err(SyncError::Extra { path: relative });
        }
    }

    Ok(())
}

fn metadata(root: &Path, path: &str) -> Result<Option<fs::Metadata>, SyncError> {
    match fs::symlink_metadata(root.join(path)) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SyncError::Read {
            path: path.to_string(),
            source,
        }),
    }
}

fn read_existing(root: &Path, path: &str) -> Result<Option<Vec<u8>>, SyncError> {
    let Some(metadata) = metadata(root, path)? else {
        return Ok(None);
    };

    if !metadata.is_file() {
        return Err(SyncError::NotAFile {
            path: path.to_string(),
        });
    }

    let bytes = fs::read(root.join(path)).map_err(|source| SyncError::Read {
        path: path.to_string(),
        source,
    })?;

    Ok(Some(bytes))
}

fn write_file(root: &Path, path: &str, content: &str) -> Result<(), SyncError> {
    let target = root.join(path);

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| SyncError::Write {
            path: path.to_string(),
            source,
        })?;
    }

    fs::write(&target, content).map_err(|source| SyncError::Write {
        path: path.to_string(),
        source,
    })
}

fn remove_file(root: &Path, path: &str) -> Result<(), SyncError> {
    match fs::remove_file(root.join(path)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SyncError::Remove {
            path: path.to_string(),
            source,
        }),
    }
}

fn write_record(root: &Path, record: &Record) -> Result<(), SyncError> {
    let text = toml::to_string(record).expect("the ownership record serializes");

    write_file(root, RECORD_FILE, &text)
}

#[cfg(test)]
mod tests;
