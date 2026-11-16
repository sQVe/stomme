use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::agents_block::{self, BlockError};
use crate::config::{self, Installation, Selection};
use crate::full_check;
use crate::layer::Catalog;
use crate::owned::{self, DOCS_FOLDER, LinkError, OwnedFile};
use crate::synced_tree;
use crate::synced_tree::WalkError;

mod record_state;

pub(crate) use record_state::{RecordState, record_state};

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
    #[error("{path} is a symlink; synced files must stay inside the project")]
    SymlinkedFolder { path: String },
    #[error("{path} differs from the synced content; edit it or remove it")]
    Edited { path: String },
    #[error("{path}: {source}")]
    Block { path: String, source: BlockError },
    #[error("{path} is not valid UTF-8")]
    NotText { path: String },
    #[error("the managed block links to {target}, which does not exist")]
    BlockLink { target: String },
    #[error("{path} is not owned by stomme; remove it")]
    Extra { path: String },
    #[error("could not read {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error("could not write {path}: {source}")]
    Write { path: String, source: io::Error },
    #[error("could not remove {path}: {source}")]
    Remove { path: String, source: io::Error },
}

type PlannedWrites = (Vec<(String, String)>, BTreeMap<String, String>);

impl From<BlockError> for SyncError {
    fn from(source: BlockError) -> Self {
        Self::Block {
            path: agents_block::PATH.to_string(),
            source,
        }
    }
}

struct Plan {
    writes: Vec<(String, String)>,
    deletes: Vec<String>,
    record: Record,
}

pub(crate) fn run(root: &Path, catalog: &Catalog) -> Result<(), SyncError> {
    let selection = match config::read(root, catalog) {
        Installation::Valid(selection) => selection,
        Installation::NoStomme => return Err(SyncError::NoStomme),
        Installation::Broken(reason) => return Err(SyncError::Broken { reason }),
    };
    let owned = owned::owned_set(&selection, catalog);

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
    check_parent_folders(root, RECORD_FILE)?;

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
        check_parent_folders(root, path)?;
        check_regular_file(root, path)?;
    }

    check_parent_folders(root, agents_block::PATH)?;
    check_regular_file(root, agents_block::PATH)?;
    check_extra_files(root, &owned_paths, &recorded_paths)?;

    let (mut writes, mut files) = plan_writes(root, owned, record.as_ref())?;

    plan_block(
        root,
        selection,
        owned,
        record.as_ref(),
        &mut writes,
        &mut files,
    )?;

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

fn plan_block(
    root: &Path,
    selection: &Selection,
    owned: &[OwnedFile],
    record: Option<&Record>,
    writes: &mut Vec<(String, String)>,
    files: &mut BTreeMap<String, String>,
) -> Result<(), SyncError> {
    let Some(existing) = read_existing(root, agents_block::PATH)? else {
        return Ok(());
    };
    let existing = String::from_utf8(existing).map_err(|_| SyncError::NotText {
        path: agents_block::PATH.to_string(),
    })?;
    let argv = full_check::read(root).ok().flatten();
    let block = agents_block::render(selection, argv.as_deref());

    validate_block_links(root, &block, owned)?;

    files.insert(agents_block::PATH.to_string(), hash(block.as_bytes()));

    let Some(range) = agents_block::find(&existing)? else {
        writes.push((
            agents_block::PATH.to_string(),
            agents_block::replace(&existing, &block)?,
        ));

        return Ok(());
    };

    let current = &existing[range];

    if current == block {
        return Ok(());
    }

    let actual = hash(current.as_bytes());

    if record_state(record, agents_block::PATH, &actual) != RecordState::Matches {
        return Err(SyncError::Edited {
            path: agents_block::PATH.to_string(),
        });
    }

    writes.push((
        agents_block::PATH.to_string(),
        agents_block::replace(&existing, &block)?,
    ));

    Ok(())
}

fn validate_block_links(root: &Path, block: &str, owned: &[OwnedFile]) -> Result<(), SyncError> {
    let owned_paths: BTreeSet<&str> = owned.iter().map(|file| file.path.as_str()).collect();

    for target in owned::link_targets(block) {
        let Some(relative) = owned::relative_target(&target) else {
            continue;
        };
        let Some(resolved) = owned::resolve("", relative) else {
            return Err(SyncError::BlockLink { target });
        };

        if owned_paths.contains(resolved.as_str()) {
            continue;
        }

        if !root.join(&resolved).is_file() {
            return Err(SyncError::BlockLink { target });
        }
    }

    Ok(())
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
            writes.push((file.path.clone(), file.content.to_string()));
            continue;
        };

        if existing == file.content.as_bytes() {
            continue;
        }

        let actual = hash(&existing);

        if record_state(record, &file.path, &actual) != RecordState::Matches {
            return Err(SyncError::Edited {
                path: file.path.clone(),
            });
        }

        writes.push((file.path.clone(), file.content.to_string()));
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

    for path in record.files.keys() {
        if owned_paths.contains(path.as_str()) || path == agents_block::PATH {
            continue;
        }

        let Some(existing) = read_existing(root, path)? else {
            continue;
        };

        let actual = hash(&existing);

        if record_state(Some(record), path, &actual) != RecordState::Matches {
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
        let allowed = inside || path == agents_block::PATH;
        let clean = !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");

        if path == RECORD_FILE || !allowed || !clean {
            return Err(SyncError::MalformedRecord {
                path: RECORD_FILE.to_string(),
                reason: format!("names `{path}`, which is not an owned file"),
            });
        }
    }

    Ok(())
}

pub(crate) fn check_parent_folders(root: &Path, path: &str) -> Result<(), SyncError> {
    let parts: Vec<&str> = path.split('/').collect();

    for end in 1..parts.len() {
        let relative = parts[..end].join("/");

        match fs::symlink_metadata(root.join(&relative)) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(SyncError::SymlinkedFolder { path: relative });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(SyncError::NotAFile { path: relative });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(SyncError::Read {
                    path: relative,
                    source,
                });
            }
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
    for item in synced_tree::files(root, RECORD_FILE) {
        let path = match item {
            Ok(path) => path,
            Err(WalkError::MissingRoot { .. }) => return Ok(()),
            Err(WalkError::Read { path, source }) => return Err(SyncError::Read { path, source }),
            Err(WalkError::NotUtf8 { path }) => return Err(SyncError::NotText { path }),
        };

        if !owned.contains(path.as_str()) && !recorded.contains(path.as_str()) {
            return Err(SyncError::Extra { path });
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
