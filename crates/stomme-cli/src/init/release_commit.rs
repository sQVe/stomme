use std::env;
use std::ffi::OsString;

use super::{InitError, VERSION};
use crate::commit::is_40_character_lowercase_hex_commit;

#[derive(Debug)]
pub(crate) struct ReleaseCommit(pub(super) String);

pub(crate) fn from_environment() -> Result<ReleaseCommit, InitError> {
    resolve(
        embedded(env!("STOMME_EMBEDDED_RELEASE_COMMIT")),
        env::var_os("STOMME_RELEASE_COMMIT"),
    )
}

pub(super) fn resolve(
    embedded: Option<&str>,
    environment: Option<OsString>,
) -> Result<ReleaseCommit, InitError> {
    if let Some(embedded) = embedded {
        return parse(embedded);
    }

    let Some(environment) = environment else {
        return Err(InitError::UnknownReleaseCommit { version: VERSION });
    };

    let value = environment.to_string_lossy().into_owned();

    parse(&value)
}

fn embedded(commit: &'static str) -> Option<&'static str> {
    // Cargo sets this from the build script, which overrides any value exported for the build.
    // Reading the process environment here would let a build shell embed an arbitrary commit.
    if commit.is_empty() {
        None
    } else {
        Some(commit)
    }
}

fn parse(value: &str) -> Result<ReleaseCommit, InitError> {
    if !is_40_character_lowercase_hex_commit(value) {
        return Err(InitError::InvalidReleaseCommit {
            value: value.to_string(),
        });
    }

    Ok(ReleaseCommit(value.to_string()))
}

#[cfg(test)]
mod tests;
