use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::owned::DOCS_FOLDER;

#[derive(Debug)]
pub(crate) enum WalkError {
    MissingRoot { path: String, source: io::Error },
    Read { path: String, source: io::Error },
    NotUtf8 { path: String },
}

pub(crate) struct Files<'a> {
    root: &'a Path,
    skip: &'a str,
    pending: Option<PathBuf>,
    folders: Vec<(PathBuf, fs::ReadDir)>,
    done: bool,
}

pub(crate) fn files<'a>(root: &'a Path, skip: &'a str) -> Files<'a> {
    Files {
        root,
        skip,
        pending: Some(root.join(DOCS_FOLDER)),
        folders: Vec::new(),
        done: false,
    }
}

impl Iterator for Files<'_> {
    type Item = Result<String, WalkError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if let Some(folder) = self.pending.take() {
            match fs::read_dir(&folder) {
                Ok(entries) => self.folders.push((folder, entries)),
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    self.done = true;
                    return Some(Err(WalkError::MissingRoot {
                        path: DOCS_FOLDER.to_string(),
                        source,
                    }));
                }
                Err(source) => {
                    self.done = true;
                    return Some(Err(WalkError::Read {
                        path: DOCS_FOLDER.to_string(),
                        source,
                    }));
                }
            }
        }

        while let Some((folder, entries)) = self.folders.last_mut() {
            let folder = folder.clone();
            let Some(entry) = entries.next() else {
                self.folders.pop();
                continue;
            };
            let entry = match entry {
                Ok(entry) => entry,
                Err(source) => {
                    self.done = true;
                    return Some(Err(WalkError::Read {
                        path: describe_relative(self.root, &folder),
                        source,
                    }));
                }
            };
            let path = entry.path();
            let Some(relative) = relative_path(self.root, &path) else {
                return Some(Err(WalkError::NotUtf8 {
                    path: describe_relative(self.root, &path),
                }));
            };

            if relative == self.skip {
                continue;
            }

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(source) => {
                    self.done = true;
                    return Some(Err(WalkError::Read {
                        path: describe_relative(self.root, &path),
                        source,
                    }));
                }
            };

            if file_type.is_dir() {
                match fs::read_dir(&path) {
                    Ok(entries) => self.folders.push((path, entries)),
                    Err(source) => {
                        self.done = true;
                        return Some(Err(WalkError::Read {
                            path: describe_relative(self.root, &path),
                            source,
                        }));
                    }
                }
            } else {
                return Some(Ok(relative));
            }
        }

        self.done = true;

        None
    }
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let mut parts = Vec::new();

    for component in path.strip_prefix(root).ok()?.components() {
        parts.push(component.as_os_str().to_str()?);
    }

    Some(parts.join("/"))
}

fn describe_relative(root: &Path, path: &Path) -> String {
    describe(path.strip_prefix(root).unwrap_or(path))
}

fn describe(path: &Path) -> String {
    #[cfg(unix)]
    {
        use std::fmt::Write as _;
        use std::os::unix::ffi::OsStrExt;

        let mut text = String::new();

        for &byte in path.as_os_str().as_bytes() {
            if byte == b'\\' {
                text.push_str("\\\\");
            } else if byte.is_ascii_graphic() || byte == b' ' {
                text.push(char::from(byte));
            } else {
                write!(&mut text, "\\x{byte:02X}").expect("writing to a string cannot fail");
            }
        }

        text
    }

    #[cfg(not(unix))]
    {
        path.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn files_returns_relative_paths_and_skips_the_record() {
        let root = tempfile::tempdir().expect("create a temporary directory");

        fs::create_dir_all(root.path().join("docs/stomme/layers")).expect("create a folder");
        fs::write(root.path().join("docs/stomme/one.md"), "one\n").expect("write a file");
        fs::write(root.path().join("docs/stomme/layers/two.md"), "two\n").expect("write a file");
        fs::write(root.path().join("docs/stomme/.sync-record.toml"), "").expect("write the record");

        let mut found = files(root.path(), "docs/stomme/.sync-record.toml")
            .collect::<Result<Vec<_>, _>>()
            .expect("walk the synced tree");

        found.sort();

        assert_eq!(found, ["docs/stomme/layers/two.md", "docs/stomme/one.md"]);
    }

    #[test]
    fn files_reports_a_missing_synced_folder() {
        let root = tempfile::tempdir().expect("create a temporary directory");
        let mut files = files(root.path(), ".sync-record.toml");

        let error = files
            .next()
            .expect("the walk yields the missing folder")
            .expect_err("the folder is missing");
        let WalkError::MissingRoot { path, source } = error else {
            panic!("expected a missing root, got {error:?}");
        };

        assert_eq!(path, "docs/stomme");
        assert_eq!(source.kind(), io::ErrorKind::NotFound);
    }
}
