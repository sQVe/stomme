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

    let record = Record::read_file(&directory.join(BUNDLE_RECORD), BUNDLE_RECORD)?;
    let (writes, deletes, hashes) = plan(directory, &files, record.as_ref())?;

    for (path, content) in writes {
        sync::write_file(directory, &path, &content)?;
    }

    for path in deletes {
        sync::remove_file(directory, &path)?;
    }

    let record = Record {
        schema: RECORD_SCHEMA,
        version: env!("CARGO_PKG_VERSION").to_string(),
        agents: None,
        files: hashes,
    };
    let text = toml::to_string(&record).expect("the bundle record serializes");

    sync::write_file(directory, BUNDLE_RECORD, &text)?;

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

type BundlePlan = (Vec<(String, String)>, Vec<String>, BTreeMap<String, String>);

fn plan(
    directory: &Path,
    files: &[OwnedFile],
    record: Option<&Record>,
) -> Result<BundlePlan, GuidesError> {
    let mut writes = Vec::new();
    let mut hashes = BTreeMap::new();

    for file in files {
        let key = sync::record_key(&file.path).expect("a bundle path is under docs/stomme");

        hashes.insert(key.to_string(), sync::hash(file.content.as_bytes()));
        sync::check_parent_folders(directory, key)?;

        match existing(directory, key)? {
            Some(bytes) if bytes == file.content.as_bytes() => {}
            Some(bytes) => {
                let actual = sync::hash(&bytes);

                match record.and_then(|record| record.files.get(key)) {
                    Some(recorded) if recorded == &actual => {
                        writes.push((key.to_string(), file.content.to_string()));
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

    let deletes = plan_deletes(directory, record, &hashes)?;

    Ok((writes, deletes, hashes))
}

fn plan_deletes(
    directory: &Path,
    record: Option<&Record>,
    hashes: &BTreeMap<String, String>,
) -> Result<Vec<String>, GuidesError> {
    let Some(record) = record else {
        return Ok(Vec::new());
    };
    let mut deletes = Vec::new();

    for (key, recorded) in &record.files {
        if hashes.contains_key(key) {
            continue;
        }

        let Some(bytes) = existing(directory, key)? else {
            continue;
        };

        if sync::hash(&bytes) != *recorded {
            return Err(GuidesError::Edited { path: key.clone() });
        }

        deletes.push(key.clone());
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
