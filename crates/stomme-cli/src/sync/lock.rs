use std::fs;
use std::path::Path;

use super::{LOCK_NAME, SyncError};

pub(crate) struct Lock {
    _file: fs::File,
}

impl Lock {
    // Blocks while another stomme run holds the lock for `folder`.
    pub(crate) fn acquire(folder: &Path, label: &str) -> Result<Self, SyncError> {
        fs::create_dir_all(folder).map_err(|source| SyncError::Write {
            path: label.to_string(),
            source,
        })?;

        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(folder.join(LOCK_NAME))
            .map_err(|source| SyncError::Lock {
                path: label.to_string(),
                source,
            })?;

        file.lock().map_err(|source| SyncError::Lock {
            path: label.to_string(),
            source,
        })?;

        Ok(Self { _file: file })
    }
}
