use std::fs;
use std::io;
use std::path::Path;

use crate::owned::DOCS_FOLDER;

#[derive(Debug)]
pub(crate) struct WalkError {
    pub(crate) path: String,
    pub(crate) source: io::Error,
}

pub(crate) fn files(root: &Path, skip: &str) -> Result<Vec<String>, WalkError> {
    let mut files = Vec::new();

    collect(root, DOCS_FOLDER, skip, &mut files)?;

    Ok(files)
}

fn collect(
    root: &Path,
    folder: &str,
    skip: &str,
    files: &mut Vec<String>,
) -> Result<(), WalkError> {
    let entries = fs::read_dir(root.join(folder)).map_err(|source| WalkError {
        path: folder.to_string(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| WalkError {
            path: folder.to_string(),
            source,
        })?;
        let path = entry.path();
        let relative = relative_path(root, &path);

        if relative == skip {
            continue;
        }

        let file_type = entry.file_type().map_err(|source| WalkError {
            path: relative.clone(),
            source,
        })?;

        if file_type.is_dir() {
            collect(root, &relative, skip, files)?;
        } else {
            files.push(relative);
        }
    }

    Ok(())
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

        let mut found =
            files(root.path(), "docs/stomme/.sync-record.toml").expect("walk the synced tree");

        found.sort();

        assert_eq!(found, ["docs/stomme/layers/two.md", "docs/stomme/one.md"]);
    }

    #[test]
    fn files_reports_a_missing_synced_folder() {
        let root = tempfile::tempdir().expect("create a temporary directory");

        let error = files(root.path(), ".sync-record.toml").expect_err("the folder is missing");

        assert_eq!(error.path, "docs/stomme");
        assert_eq!(error.source.kind(), io::ErrorKind::NotFound);
    }
}
