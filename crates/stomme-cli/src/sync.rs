use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::agents_block::{self, BlockError};
use crate::config::{self, Installation, Selection};
use crate::full_check;
use crate::layer::Catalog;
use crate::ocr_rules::{self, OcrRulesError};
use crate::owned::{self, LinkError, OwnedFile};
use crate::synced_tree;
use crate::synced_tree::WalkError;

mod lock;
mod package_pins;
mod record_state;

pub(crate) use lock::Lock;
pub(crate) use record_state::{RecordState, record_state};

pub(crate) const RECORD_FILE: &str = "docs/stomme/.sync-record.toml";
pub(crate) const RECORD_SCHEMA: i64 = 1;

const STAGING_PREFIX: &str = ".stomme-staging.";
const STAGING_ATTEMPTS: u32 = 16;
pub(crate) const LOCK_NAME: &str = ".sync-lock";
pub(crate) const LOCK_FILE: &str = "docs/stomme/.sync-lock";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub(crate) schema: i64,
    pub(crate) version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agents: Option<String>,
    #[serde(default)]
    pub(crate) files: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviousRecord {
    version: String,
    #[serde(default)]
    files: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub(crate) enum SyncError {
    #[error("stomme.toml is missing; add one with `schema = 1`, `version`, and a `language`")]
    NoStomme,
    #[error("stomme.toml {reason}")]
    Broken { reason: config::Reason },
    #[error(transparent)]
    Link(#[from] LinkError),
    #[error(transparent)]
    OcrRules(#[from] OcrRulesError),
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
    #[error("package.json {reason}")]
    PackageManifest { reason: String },
    #[error("could not read {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error("could not lock {path}: {source}")]
    Lock { path: String, source: io::Error },
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
    staging: Vec<String>,
    deletes: Vec<String>,
    record: Record,
}

pub(crate) fn run(
    root: &Path,
    catalog: &Catalog,
    file: &config::FileRead,
) -> Result<(), SyncError> {
    let selection = match config::read(root, catalog, file) {
        Installation::Valid(selection) => selection,
        Installation::NoStomme => return Err(SyncError::NoStomme),
        Installation::Broken(reason) => return Err(SyncError::Broken { reason }),
    };
    let project = ocr_rules::read_project(root)?;
    let owned = owned::owned_set(&selection, catalog, project.as_ref());

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
        Self::read_file(&root.join(RECORD_FILE), RECORD_FILE)
    }

    pub(crate) fn read_file(path: &Path, label: &str) -> Result<Option<Self>, SyncError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(malformed(label, "is not a regular file".to_string()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(SyncError::Read {
                    path: label.to_string(),
                    source,
                });
            }
        }

        let text = fs::read_to_string(path).map_err(|source| SyncError::Read {
            path: label.to_string(),
            source,
        })?;
        let value: toml::Value = toml::from_str(&text)
            .map_err(|error: toml::de::Error| malformed(label, error.message().to_string()))?;
        let record = if value.get("schema").is_some() {
            let record: Record = toml::from_str(&text)
                .map_err(|error: toml::de::Error| malformed(label, error.message().to_string()))?;

            if record.schema != RECORD_SCHEMA {
                return Err(malformed(
                    label,
                    format!(
                        "records schema {}; this stomme reads schema {RECORD_SCHEMA}",
                        record.schema
                    ),
                ));
            }

            record
        } else {
            let previous: PreviousRecord = toml::from_str(&text)
                .map_err(|error: toml::de::Error| malformed(label, error.message().to_string()))?;

            previous.into_current(label)?
        };

        record.validate(label)?;

        Ok(Some(record))
    }

    pub(crate) fn validate(&self, label: &str) -> Result<(), SyncError> {
        for path in self.files.keys() {
            if !valid_key(path) {
                return Err(malformed(
                    label,
                    format!("names `{path}`, which is not a synced file"),
                ));
            }
        }

        Ok(())
    }
}

impl PreviousRecord {
    fn into_current(self, label: &str) -> Result<Record, SyncError> {
        let mut files = BTreeMap::new();
        let mut agents = None;

        for (path, digest) in self.files {
            if path == agents_block::PATH {
                agents = Some(digest);
                continue;
            }

            let Some(key) = owned::record_key(&path) else {
                return Err(malformed(
                    label,
                    format!("names `{path}`, which is not a synced file"),
                ));
            };

            if !valid_key(key) {
                return Err(malformed(
                    label,
                    format!("names `{path}`, which is not a synced file"),
                ));
            }

            files.insert(key.to_string(), digest);
        }

        Ok(Record {
            schema: RECORD_SCHEMA,
            version: self.version,
            agents,
            files,
        })
    }
}

fn malformed(path: &str, reason: String) -> SyncError {
    SyncError::MalformedRecord {
        path: path.to_string(),
        reason,
    }
}

fn valid_key(key: &str) -> bool {
    let reserved = key == ".sync-record.toml" || key == LOCK_NAME || key == agents_block::PATH;
    let clean = !key.starts_with('/')
        && !key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");

    !reserved && clean
}

fn sync(root: &Path, selection: &Selection, owned: &[OwnedFile]) -> Result<(), SyncError> {
    owned::validate_links(owned)?;

    // The lock lives in the folder, so refuse a symlinked folder before creating it.
    check_parent_folders(root, RECORD_FILE)?;

    let _lock = Lock::acquire(&root.join(owned::DOCS_FOLDER), LOCK_FILE)?;
    let plan = plan(root, selection, owned)?;

    execute(root, plan)
}

fn plan(root: &Path, selection: &Selection, owned: &[OwnedFile]) -> Result<Plan, SyncError> {
    let record = Record::read(root)?;

    let owned_paths: BTreeSet<&str> = owned.iter().map(|file| file.path.as_str()).collect();
    let owned_keys: BTreeSet<&str> = owned
        .iter()
        .filter_map(|file| owned::record_key(&file.path))
        .collect();
    let recorded_keys: BTreeSet<&str> = record
        .as_ref()
        .map(|record| record.files.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let recorded_paths: BTreeSet<String> = recorded_keys
        .iter()
        .map(|key| owned::project_path(key))
        .collect();

    for path in owned_paths
        .iter()
        .copied()
        .chain(recorded_paths.iter().map(String::as_str))
    {
        check_parent_folders(root, path)?;
        check_regular_file(root, path)?;
    }

    check_parent_folders(root, agents_block::PATH)?;
    check_regular_file(root, agents_block::PATH)?;
    let mut staging: BTreeSet<String> = check_extra_files(root, &owned_paths, &recorded_paths)?
        .into_iter()
        .collect();

    let (mut writes, files) = plan_writes(root, owned, record.as_ref())?;
    let agents = plan_block(root, selection, owned, record.as_ref(), &mut writes)?;

    if let Some(write) = package_pins::plan(root)? {
        writes.push(write);
    }

    staging.extend(stale_staging_beside(
        root,
        writes.iter().map(|(path, _)| path.as_str()),
    )?);

    let deletes = plan_deletes(root, record.as_ref(), &owned_keys)?;
    let record = Record {
        schema: RECORD_SCHEMA,
        version: selection.version.clone(),
        agents,
        files,
    };

    Ok(Plan {
        writes,
        staging: staging.into_iter().collect(),
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
) -> Result<Option<String>, SyncError> {
    let Some(existing) = read_existing(root, agents_block::PATH)? else {
        return Ok(None);
    };
    let existing = String::from_utf8(existing).map_err(|_| SyncError::NotText {
        path: agents_block::PATH.to_string(),
    })?;
    let argv = full_check::read(root).ok().flatten();
    let block = agents_block::render(selection, argv.as_deref());

    validate_block_links(root, &block, owned)?;

    let digest = hash(block.as_bytes());

    let Some(range) = agents_block::find(&existing)? else {
        writes.push((
            agents_block::PATH.to_string(),
            agents_block::replace(&existing, &block)?,
        ));

        return Ok(Some(digest));
    };

    let current = &existing[range];

    if current == block {
        return Ok(Some(digest));
    }

    let actual = hash(current.as_bytes());

    if record.and_then(|record| record.agents.as_deref()) != Some(actual.as_str()) {
        return Err(SyncError::Edited {
            path: agents_block::PATH.to_string(),
        });
    }

    writes.push((
        agents_block::PATH.to_string(),
        agents_block::replace(&existing, &block)?,
    ));

    Ok(Some(digest))
}

fn validate_block_links(root: &Path, block: &str, owned: &[OwnedFile]) -> Result<(), SyncError> {
    let owned_paths: BTreeSet<&str> = owned.iter().map(|file| file.path.as_str()).collect();

    for target in owned::link_targets(block) {
        let Some(link) = owned::relative_link(&target) else {
            continue;
        };
        let Some(resolved) = owned::resolve("", link.path) else {
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
        let key = owned::record_key(&file.path).expect("an owned path is under docs/stomme");

        files.insert(key.to_string(), hash(file.content.as_bytes()));

        let Some(existing) = read_existing(root, &file.path)? else {
            writes.push((file.path.clone(), file.content.to_string()));
            continue;
        };

        if existing == file.content.as_bytes() {
            continue;
        }

        let actual = hash(&existing);

        if record_state(record, key, &actual) != RecordState::Matches {
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
    owned_keys: &BTreeSet<&str>,
) -> Result<Vec<String>, SyncError> {
    let Some(record) = record else {
        return Ok(Vec::new());
    };
    let mut deletes = Vec::new();

    for key in record.files.keys() {
        if owned_keys.contains(key.as_str()) {
            continue;
        }

        let path = owned::project_path(key);
        let Some(existing) = read_existing(root, &path)? else {
            continue;
        };

        let actual = hash(&existing);

        if record_state(Some(record), key, &actual) != RecordState::Matches {
            return Err(SyncError::Edited { path });
        }

        deletes.push(path);
    }

    Ok(deletes)
}

fn execute(root: &Path, plan: Plan) -> Result<(), SyncError> {
    for path in &plan.staging {
        remove_file(root, path)?;
    }

    for (path, content) in &plan.writes {
        write_file(root, path, content)?;
    }

    for path in &plan.deletes {
        remove_file(root, path)?;
    }

    write_record(root, &plan.record)
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
    recorded: &BTreeSet<String>,
) -> Result<Vec<String>, SyncError> {
    let mut staging = Vec::new();

    for path in synced_files(root)? {
        if owned.contains(path.as_str()) || recorded.contains(&path) {
            continue;
        }

        if is_staging(&path) {
            staging.push(path);
            continue;
        }

        return Err(SyncError::Extra { path });
    }

    Ok(staging)
}

pub(crate) fn stale_staging_beside<'a>(
    base: &Path,
    paths: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<String>, SyncError> {
    let mut folders = BTreeSet::new();
    let mut staging = BTreeSet::new();

    folders.insert(base.to_path_buf());

    for path in paths {
        if let Some(folder) = Path::new(path).parent() {
            folders.insert(base.join(folder));
        }
    }

    for folder in folders {
        staging.extend(stale_staging_in(base, &folder)?);
    }

    Ok(staging.into_iter().collect())
}

fn stale_staging_in(base: &Path, folder: &Path) -> Result<Vec<String>, SyncError> {
    let mut paths = Vec::new();

    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(paths),
        Err(source) => {
            return Err(SyncError::Read {
                path: synced_tree::describe_relative(base, folder),
                source,
            });
        }
    };

    for entry in entries {
        let entry = entry.map_err(|source| SyncError::Read {
            path: synced_tree::describe_relative(base, folder),
            source,
        })?;

        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };

        if !is_staging_name(&name) {
            continue;
        }

        let file_type = entry.file_type().map_err(|source| SyncError::Read {
            path: synced_tree::describe_relative(base, &entry.path()),
            source,
        })?;

        if !file_type.is_file() {
            continue;
        }

        let Some(relative) = synced_tree::relative_path(base, &entry.path()) else {
            continue;
        };

        paths.push(relative);
    }

    Ok(paths)
}

fn is_staging(path: &str) -> bool {
    path.rsplit('/').next().is_some_and(is_staging_name)
}

fn is_staging_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(STAGING_PREFIX) else {
        return false;
    };

    let mut parts = rest.split('.');
    let process = parts.next().and_then(|part| part.parse::<u32>().ok());
    let attempt = parts.next().and_then(|part| part.parse::<u32>().ok());
    let has_name = parts.next().is_some();

    process.is_some() && attempt.is_some() && has_name
}

fn synced_files(root: &Path) -> Result<Vec<String>, SyncError> {
    let mut paths = Vec::new();

    for item in synced_tree::files(root, &[RECORD_FILE, LOCK_FILE]) {
        match item {
            Ok(path) => paths.push(path),
            Err(WalkError::MissingRoot { .. }) => return Ok(Vec::new()),
            Err(WalkError::Read { path, source }) => return Err(SyncError::Read { path, source }),
            Err(WalkError::NotUtf8 { path }) => return Err(SyncError::NotText { path }),
        }
    }

    Ok(paths)
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

pub(crate) fn write_file(root: &Path, path: &str, content: &str) -> Result<(), SyncError> {
    // Publishing an unchanged target would move its modification time, which file watchers report
    // as a change.
    let unchanged = read_existing(root, path)?.is_some_and(|bytes| bytes == content.as_bytes());

    if unchanged {
        return Ok(());
    }

    let target = root.join(path);

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| SyncError::Write {
            path: path.to_string(),
            source,
        })?;
    }

    let staging = stage(root, &target, content, path)?;

    staging.publish(&target, path)
}

fn stage(root: &Path, target: &Path, content: &str, path: &str) -> Result<Staging, SyncError> {
    let permissions = writable_permissions(root, path)?;
    let (staging, mut file) = create_staging(target, permissions.as_ref(), path)?;
    let written = file
        .write_all(content.as_bytes())
        .and_then(|()| file.sync_all());

    drop(file);

    written.map_err(|source| SyncError::Write {
        path: path.to_string(),
        source,
    })?;

    Ok(staging)
}

fn writable_permissions(root: &Path, path: &str) -> Result<Option<fs::Permissions>, SyncError> {
    let Some(metadata) = metadata(root, path)? else {
        return Ok(None);
    };

    if !metadata.is_file() {
        return Ok(None);
    }

    // Sync replaces the target instead of writing into it, which would succeed on a file the user
    // made read-only. Probing the open keeps the refusal the in-place write gave.
    fs::OpenOptions::new()
        .write(true)
        .open(root.join(path))
        .map_err(|source| SyncError::Write {
            path: path.to_string(),
            source,
        })?;

    Ok(Some(metadata.permissions()))
}

fn create_staging(
    target: &Path,
    permissions: Option<&fs::Permissions>,
    path: &str,
) -> Result<(Staging, fs::File), SyncError> {
    let folder = target.parent().expect("a synced path has a parent folder");
    let process = std::process::id();

    for attempt in 0..STAGING_ATTEMPTS {
        let staging = folder.join(staging_name(target, process, attempt));

        let Some(file) = create_exclusive(&staging, permissions, path)? else {
            continue;
        };

        let staging = Staging { path: staging };

        // The umask can clear bits the creation mode asked for.
        if let Some(permissions) = permissions
            && let Err(source) = file.set_permissions(permissions.clone())
        {
            drop(file);

            return Err(SyncError::Write {
                path: path.to_string(),
                source,
            });
        }

        return Ok((staging, file));
    }

    Err(SyncError::Write {
        path: path.to_string(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "every staging name in the folder is taken",
        ),
    })
}

fn staging_name(target: &Path, process: u32, attempt: u32) -> String {
    let name = target
        .file_name()
        .expect("a synced path names a file")
        .to_string_lossy();

    format!("{STAGING_PREFIX}{process}.{attempt}.{name}")
}

fn create_exclusive(
    staging: &Path,
    permissions: Option<&fs::Permissions>,
    path: &str,
) -> Result<Option<fs::File>, SyncError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    if let Some(permissions) = permissions {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

        options.mode(permissions.mode() & 0o7777);
    }

    let file = match options.open(staging) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(None),
        Err(source) => {
            return Err(SyncError::Write {
                path: path.to_string(),
                source,
            });
        }
    };

    Ok(Some(file))
}

struct Staging {
    path: PathBuf,
}

impl Staging {
    fn publish(self, target: &Path, path: &str) -> Result<(), SyncError> {
        fs::rename(&self.path, target).map_err(|source| SyncError::Write {
            path: path.to_string(),
            source,
        })
    }
}

impl Drop for Staging {
    #[expect(
        clippy::print_stderr,
        reason = "no caller can take the cleanup error after a failed write"
    )]
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!(
                "could not remove the staging file {}: {error}",
                self.path.display()
            );
        }
    }
}

pub(crate) fn remove_file(root: &Path, path: &str) -> Result<(), SyncError> {
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
