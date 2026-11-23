use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use thiserror::Error;

use crate::config::FileRead;
use crate::layer::{Catalog, Language, SelectionError};
use crate::sync::{self, SyncError};
use assets::Source;

mod assets;
mod merge;
mod plan;
mod release_commit;

const NAME_FLAG: &str = "--name";
const LANGUAGE_FLAG: &str = "--language";
const LAYERS_FLAG: &str = "--layers";
const ROLES_FLAG: &str = "--roles";
const FLAGS: [&str; 4] = [NAME_FLAG, LANGUAGE_FLAG, LAYERS_FLAG, ROLES_FLAG];

// npm's package-name limit, which also keeps `crates/<name>-<role>` under the 255-byte file-name
// limit that common filesystems impose.
const NAME_LENGTH_LIMIT: usize = 214;
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Request {
    name: String,
    language: Language,
    layers: Vec<String>,
    roles: Vec<String>,
}

pub(crate) use release_commit::ReleaseCommit;

#[derive(Debug, Error)]
pub(crate) enum InitError {
    #[error("init has no flag `{flag}`")]
    UnknownFlag { flag: String },
    #[error("lists flag `{flag}` twice")]
    RepeatedFlag { flag: String },
    #[error("the flag `{flag}` has no value")]
    MissingValue { flag: String },
    #[error("init needs the flag `{flag}`")]
    MissingFlag { flag: &'static str },
    #[error("the language `{language}` is not `rust` or `typescript`")]
    UnknownLanguage { language: String },
    #[error(
        "the name `{name}` is not a lower-case letter followed by lower-case letters, digits, or `-`"
    )]
    Name { name: String },
    #[error("the name is longer than {NAME_LENGTH_LIMIT} characters")]
    NameTooLong,
    #[error("the name `{name}` collides with the generated `{name}` package; choose another name")]
    NameCollision { name: String },
    #[error(transparent)]
    Selection(#[from] SelectionError),
    #[error("role `{role}` has no starter for {language}")]
    UnknownRole { role: String, language: Language },
    #[error("lists role `{role}` twice")]
    DuplicateRole { role: String },
    #[error("{path} is not empty; init writes into an empty folder")]
    NotEmpty { path: String },
    #[error("`{first}` and `{second}` both own `{path}`")]
    OwnerClash {
        path: String,
        first: String,
        second: String,
    },
    #[error("`{first}` and `{second}` set `{path}` to different values")]
    MergeClash {
        path: String,
        first: String,
        second: String,
    },
    #[error("`{path}` is not a JSON or TOML file, so its fragments cannot merge")]
    MergeTarget { path: String },
    #[error("`{owner}` gives `{path}` a fragment that does not parse: {reason}")]
    InvalidFragment {
        path: String,
        owner: String,
        reason: String,
    },
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error("could not write {path}: {source}")]
    Write { path: String, source: io::Error },
    #[error("could not read {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error(
        "this stomme build does not know the commit of release v{version}; install a released binary, or set STOMME_RELEASE_COMMIT to that commit"
    )]
    UnknownReleaseCommit { version: &'static str },
    #[error("STOMME_RELEASE_COMMIT `{value}` is not a 40-character lowercase hex commit")]
    InvalidReleaseCommit { value: String },
}

pub(crate) fn parse_arguments(arguments: &[String]) -> Result<Request, InitError> {
    let mut values: BTreeMap<&str, &str> = BTreeMap::new();
    let mut remaining = arguments.iter();

    while let Some(flag) = remaining.next() {
        let Some(known) = FLAGS.into_iter().find(|known| known == flag) else {
            return Err(InitError::UnknownFlag { flag: flag.clone() });
        };
        let Some(value) = remaining.next() else {
            return Err(InitError::MissingValue { flag: flag.clone() });
        };

        if values.insert(known, value).is_some() {
            return Err(InitError::RepeatedFlag { flag: flag.clone() });
        }
    }

    let name = required(&values, NAME_FLAG)?;
    let language = required(&values, LANGUAGE_FLAG)?;
    let language = Language::parse(language).ok_or_else(|| InitError::UnknownLanguage {
        language: language.to_string(),
    })?;

    if !is_valid_name(name) {
        return Err(InitError::Name {
            name: name.to_string(),
        });
    }

    if name.chars().count() > NAME_LENGTH_LIMIT {
        return Err(InitError::NameTooLong);
    }

    Ok(Request {
        name: name.to_string(),
        language,
        layers: list(values.get(LAYERS_FLAG)),
        roles: list(values.get(ROLES_FLAG)),
    })
}

pub(crate) fn run(
    root: &Path,
    catalog: &Catalog,
    request: &Request,
    file: &FileRead,
    commit: impl Fn() -> Result<ReleaseCommit, InitError>,
) -> Result<(), InitError> {
    let sources = assets::sources(request, catalog)?;
    let sources: Vec<&Source> = sources.iter().collect();

    run_with_sources(root, catalog, request, &sources, file, commit)
}

pub(crate) fn run_with_sources(
    root: &Path,
    catalog: &Catalog,
    request: &Request,
    sources: &[&Source],
    file: &FileRead,
    commit: impl Fn() -> Result<ReleaseCommit, InitError>,
) -> Result<(), InitError> {
    plan::validate(root, catalog, request)?;

    let commit = commit()?;
    let plan = plan::plan(request, sources, &commit)?;

    plan::execute(root, &plan)?;
    sync::run(root, catalog, file)?;

    Ok(())
}

pub(crate) fn release_commit_from_environment() -> Result<ReleaseCommit, InitError> {
    release_commit::from_environment()
}

fn required<'a>(
    values: &BTreeMap<&str, &'a str>,
    flag: &'static str,
) -> Result<&'a str, InitError> {
    values
        .get(flag)
        .copied()
        .ok_or(InitError::MissingFlag { flag })
}

fn is_valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    first.is_ascii_lowercase()
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn list(value: Option<&&str>) -> Vec<String> {
    match value {
        None | Some(&"") => Vec::new(),
        Some(value) => value.split(',').map(str::to_string).collect(),
    }
}

#[cfg(test)]
pub(crate) fn request(language: Language, layers: &[&str], roles: &[&str]) -> Request {
    Request {
        name: "demo".to_string(),
        language,
        layers: layers.iter().map(ToString::to_string).collect(),
        roles: roles.iter().map(ToString::to_string).collect(),
    }
}

#[cfg(test)]
pub(crate) const TEST_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

#[cfg(test)]
pub(crate) fn test_commit() -> ReleaseCommit {
    release_commit::resolve(Some(TEST_COMMIT), None).expect("the test commit is valid")
}

#[cfg(test)]
pub(crate) fn planned(request: &Request, sources: &[&Source]) -> plan::InitPlan {
    plan::plan(request, sources, &test_commit()).expect("the plan succeeds")
}

#[cfg(test)]
pub(crate) fn empty_folder() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("stomme-init-")
        .tempdir()
        .expect("create a temporary directory")
}

#[cfg(test)]
pub(crate) fn entries(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .expect("read the target folder")
        .map(|entry| {
            let entry = entry.expect("read a folder entry");

            entry.file_name().to_string_lossy().into_owned()
        })
        .collect();

    names.sort();

    names
}

#[cfg(test)]
mod tests;
