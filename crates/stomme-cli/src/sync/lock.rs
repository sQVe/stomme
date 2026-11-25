use std::fs;
use std::io;
use std::path::Path;

use super::{LOCK_NAME, SyncError};

pub(crate) struct Lock {
    _file: fs::File,
}

impl Lock {
    // Blocks while another stomme run holds the lock for `folder`.
    #[expect(
        clippy::print_stderr,
        reason = "the wait can outlast a reader's patience, and no caller prints a hint"
    )]
    pub(crate) fn acquire(folder: &Path, label: &str) -> Result<Self, SyncError> {
        fs::create_dir_all(folder).map_err(|source| SyncError::Write {
            path: label.to_string(),
            source,
        })?;

        let path = folder.join(LOCK_NAME);

        // A stat failure leaves the open below to report it.
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && !metadata.is_file()
        {
            return Err(SyncError::NotAFile {
                path: label.to_string(),
            });
        }

        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| SyncError::Lock {
                path: label.to_string(),
                source,
            })?;

        let wait = |source: io::Error| SyncError::Lock {
            path: label.to_string(),
            source,
        };

        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                eprintln!("stomme: waiting for another run to finish with {label}");

                file.lock().map_err(wait)?;
            }
            Err(fs::TryLockError::Error(source)) => return Err(wait(source)),
        }

        Ok(Self { _file: file })
    }
}
