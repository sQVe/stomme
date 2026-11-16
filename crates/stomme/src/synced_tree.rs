use std::fs;
use std::io;
use std::path::Path;

use crate::owned::DOCS_FOLDER;

#[derive(Debug)]
pub(crate) struct WalkError {
    pub(crate) path: String,
    pub(crate) source: io::Error,
}

pub(crate) struct Files<'a> {
    root: &'a Path,
    skip: &'a str,
    pending: Option<String>,
    folders: Vec<(String, fs::ReadDir)>,
    done: bool,
}

pub(crate) fn files<'a>(root: &'a Path, skip: &'a str) -> Files<'a> {
    Files {
        root,
        skip,
        pending: Some(DOCS_FOLDER.to_string()),
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
            match fs::read_dir(self.root.join(&folder)) {
                Ok(entries) => self.folders.push((folder, entries)),
                Err(source) => {
                    self.done = true;
                    return Some(Err(WalkError {
                        path: folder,
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
                    return Some(Err(WalkError {
                        path: folder,
                        source,
                    }));
                }
            };
            let path = entry.path();
            let relative = relative_path(self.root, &path);

            if relative == self.skip {
                continue;
            }

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(source) => {
                    self.done = true;
                    return Some(Err(WalkError {
                        path: relative,
                        source,
                    }));
                }
            };

            if file_type.is_dir() {
                match fs::read_dir(self.root.join(&relative)) {
                    Ok(entries) => self.folders.push((relative, entries)),
                    Err(source) => {
                        self.done = true;
                        return Some(Err(WalkError {
                            path: relative,
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

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("a docs/stomme entry lies inside the root")
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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

        assert_eq!(error.path, "docs/stomme");
        assert_eq!(error.source.kind(), io::ErrorKind::NotFound);
    }
}
