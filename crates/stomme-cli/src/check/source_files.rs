use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::layer::Language;
use crate::synced_tree::{describe_relative, relative_path};

const SKIP_FOLDERS: &[&str] = &["target", "node_modules", "vendor", "dist", "fixtures"];

pub(crate) struct Tree {
    root: PathBuf,
    pub(super) files: Vec<SourceFile>,
    pub(super) folders: BTreeSet<String>,
}

pub(crate) struct SourceFile {
    pub(super) path: String,
    pub(super) name: String,
    pub(super) stem: String,
    pub(super) folder: String,
}

impl SourceFile {
    pub(super) fn is_tsx(&self) -> bool {
        self.name.ends_with(".tsx")
    }

    pub(super) fn is_ts(&self) -> bool {
        self.name.ends_with(".ts") && !self.name.ends_with(".d.ts")
    }
}

pub(super) fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_string()
    } else {
        format!("{folder}/{name}")
    }
}

impl Tree {
    pub(super) fn read(&self, file: &SourceFile) -> anyhow::Result<String> {
        fs::read_to_string(self.root.join(&file.path))
            .with_context(|| format!("Could not read {}", file.path))
    }

    pub(super) fn source_folders(&self) -> BTreeSet<&str> {
        let mut folders = BTreeSet::new();

        for file in &self.files {
            let mut folder = file.folder.as_str();

            loop {
                folders.insert(folder);

                let Some((parent, _)) = folder.rsplit_once('/') else {
                    break;
                };

                folder = parent;
            }
        }

        folders
    }
}

pub(super) fn walk(root: &Path, language: Language) -> anyhow::Result<Tree> {
    let mut files = Vec::new();
    let mut folders = BTreeSet::new();
    let mut pending: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(folder) = pending.pop() {
        let relative_folder = relative_path(root, &folder).unwrap_or_default();
        folders.insert(relative_folder);
        let entries = fs::read_dir(&folder)
            .with_context(|| format!("Could not read {}", describe_relative(root, &folder)))?;

        for entry in entries {
            let entry = entry
                .with_context(|| format!("Could not read {}", describe_relative(root, &folder)))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("Could not read {}", describe_relative(root, &path)))?;

            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_dir() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    return Err(not_utf8(root, &path));
                };

                if name.starts_with('.') || SKIP_FOLDERS.contains(&name) {
                    continue;
                }

                pending.push(path);
            } else if file_type.is_file() {
                let Some(relative) = relative_path(root, &path) else {
                    return Err(not_utf8(root, &path));
                };

                if let Some(file) = source_file(relative, language)? {
                    files.push(file);
                }
            }
        }
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(Tree {
        root: root.to_path_buf(),
        files,
        folders,
    })
}

fn source_file(path: String, language: Language) -> anyhow::Result<Option<SourceFile>> {
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();

    if !matches_language(&name, language) {
        return Ok(None);
    }

    let stem = name.split('.').next().unwrap_or(&name).to_string();
    let folder = path
        .rsplit_once('/')
        .map_or(String::new(), |(folder, _)| folder.to_string());

    Ok(Some(SourceFile {
        path,
        name,
        stem,
        folder,
    }))
}

fn matches_language(name: &str, language: Language) -> bool {
    match language {
        Language::Rust => name.ends_with(".rs"),
        Language::TypeScript => {
            (name.ends_with(".ts") || name.ends_with(".tsx")) && !name.ends_with(".d.ts")
        }
    }
}

fn not_utf8(root: &Path, path: &Path) -> anyhow::Error {
    anyhow::anyhow!("{} is not valid UTF-8", describe_relative(root, path))
}
