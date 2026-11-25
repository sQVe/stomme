use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use thiserror::Error;

use crate::layer::Catalog;
use crate::owned::{self, LinkError, OwnedFile};
use crate::sync::{self, RECORD_SCHEMA, Record, SyncError};

pub(crate) const BUNDLE_RECORD: &str = ".sync-record.toml";

#[derive(Debug, Error)]
pub(crate) enum GuidesError {
    #[error(transparent)]
    Link(#[from] LinkError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error("{path} is not a directory")]
    NotADirectory { path: String },
    #[error("{path} is not a regular file")]
    NotAFile { path: String },
    #[error("{path} differs from the bundle's record; edit it or remove it")]
    Edited { path: String },
    #[error("{path} is not in the bundle's record; remove it")]
    Unrecorded { path: String },
    #[error("could not read {path}: {source}")]
    Read { path: String, source: io::Error },
}

pub(crate) fn run(directory: &Path, catalog: &Catalog) -> Result<(), GuidesError> {
    let files = owned::bundle_set(catalog);

    owned::validate_links(&files)?;
    check_directory(directory)?;

    let label = directory.join(sync::LOCK_NAME).display().to_string();
    let _lock = sync::Lock::acquire(directory, &label)?;
    let record = Record::read_file(&directory.join(BUNDLE_RECORD), BUNDLE_RECORD)?;
    let plan = plan(directory, &files, record.as_ref())?;

    for path in &plan.stale {
        sync::remove_file(directory, path)?;
    }

    for (path, content) in &plan.writes {
        sync::write_file(directory, path, content)?;
    }

    for path in &plan.deletes {
        sync::remove_file(directory, path)?;
    }

    let record = Record {
        schema: RECORD_SCHEMA,
        version: env!("CARGO_PKG_VERSION").to_string(),
        agents: None,
        files: plan.hashes,
    };
    let text = toml::to_string(&record).expect("the bundle record serializes");

    write_record(directory, &text)?;

    Ok(())
}

fn write_record(directory: &Path, text: &str) -> Result<(), GuidesError> {
    let path = directory.join(BUNDLE_RECORD);
    let unchanged = fs::read(&path).is_ok_and(|bytes| bytes == text.as_bytes());

    if unchanged {
        return Ok(());
    }

    sync::write_file(directory, BUNDLE_RECORD, text)?;

    Ok(())
}

fn check_directory(directory: &Path) -> Result<(), GuidesError> {
    let path = directory.display().to_string();

    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(GuidesError::NotADirectory { path }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(GuidesError::Read { path, source }),
    }
}

struct Plan {
    writes: Vec<(String, String)>,
    stale: Vec<String>,
    deletes: Vec<String>,
    hashes: BTreeMap<String, String>,
}

fn plan(
    directory: &Path,
    files: &[OwnedFile],
    record: Option<&Record>,
) -> Result<Plan, GuidesError> {
    let mut writes = Vec::new();
    let mut hashes = BTreeMap::new();

    for file in files {
        let key = owned::record_key(&file.path).expect("a bundle path is under docs/stomme");

        hashes.insert(key.to_string(), sync::hash(file.content.as_bytes()));
        sync::check_parent_folders(directory, key)?;

        match existing(directory, key)? {
            Some(bytes) => {
                let actual = sync::hash(&bytes);

                match record.and_then(|record| record.files.get(key)) {
                    Some(recorded) if recorded == &actual => {
                        if bytes != file.content.as_bytes() {
                            writes.push((key.to_string(), file.content.to_string()));
                        }
                    }
                    Some(_) => {
                        return Err(GuidesError::Edited {
                            path: key.to_string(),
                        });
                    }
                    None => {
                        return Err(GuidesError::Unrecorded {
                            path: key.to_string(),
                        });
                    }
                }
            }
            None => writes.push((key.to_string(), file.content.to_string())),
        }
    }

    let record_keys = record.into_iter().flat_map(|record| record.files.keys());
    let stale = sync::stale_staging_beside(
        directory,
        hashes.keys().chain(record_keys).map(String::as_str),
    )?;
    let deletes = plan_deletes(directory, record, &hashes)?;

    Ok(Plan {
        writes,
        stale,
        deletes,
        hashes,
    })
}

fn plan_deletes(
    directory: &Path,
    record: Option<&Record>,
    hashes: &BTreeMap<String, String>,
) -> Result<Vec<String>, GuidesError> {
    let Some(record) = record else {
        return Ok(Vec::new());
    };
    let mut candidates = Vec::new();

    for key in record.files.keys() {
        if hashes.contains_key(key) {
            continue;
        }

        sync::check_parent_folders(directory, key)?;
        candidates.push(key);
    }

    let mut deletes = Vec::new();

    for key in candidates {
        let Some(bytes) = existing(directory, key)? else {
            continue;
        };
        let recorded = &record.files[key];

        if sync::hash(&bytes) != *recorded {
            return Err(GuidesError::Edited {
                path: key.to_string(),
            });
        }

        deletes.push(key.to_string());
    }

    Ok(deletes)
}

fn existing(directory: &Path, key: &str) -> Result<Option<Vec<u8>>, GuidesError> {
    let path = directory.join(key);

    match fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.is_file() => Err(GuidesError::NotAFile {
            path: key.to_string(),
        }),
        Ok(_) => fs::read(&path)
            .map(Some)
            .map_err(|source| GuidesError::Read {
                path: key.to_string(),
                source,
            }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(GuidesError::Read {
            path: key.to_string(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests;
