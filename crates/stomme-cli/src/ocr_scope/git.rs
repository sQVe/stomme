use std::collections::BTreeMap;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use super::OcrScopeError;

pub(super) struct Git<'a> {
    pub(super) root: &'a Path,
}

impl Git<'_> {
    pub(super) fn run(&self, arguments: &[&str]) -> Result<Output, OcrScopeError> {
        self.spawn(arguments, None, &[])
    }

    pub(super) fn run_with_input(
        &self,
        arguments: &[&str],
        input: &[u8],
    ) -> Result<Output, OcrScopeError> {
        self.spawn(arguments, Some(input), &[])
    }

    pub(super) fn run_with_environment(
        &self,
        arguments: &[&str],
        environment: &[(&str, &str)],
    ) -> Result<Output, OcrScopeError> {
        self.spawn(arguments, None, environment)
    }

    fn spawn(
        &self,
        arguments: &[&str],
        input: Option<&[u8]>,
        environment: &[(&str, &str)],
    ) -> Result<Output, OcrScopeError> {
        let mut command = Command::new("git");

        command
            .args(["-c", "core.quotePath=false", "--no-pager"])
            .args(arguments)
            .current_dir(self.root)
            .envs(environment.iter().copied())
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| spawn_error(arguments, error))?;

        if let Some(input) = input {
            let mut stdin = child.stdin.take().expect("stdin is piped for an input");
            stdin
                .write_all(input)
                .map_err(|error| spawn_error(arguments, error))?;
        }

        child
            .wait_with_output()
            .map_err(|error| spawn_error(arguments, error))
    }
}

fn spawn_error(arguments: &[&str], error: io::Error) -> OcrScopeError {
    OcrScopeError::Git {
        command: arguments.join(" "),
        stderr: error.to_string(),
    }
}

pub(super) fn malformed(command: &str) -> OcrScopeError {
    OcrScopeError::Git {
        command: command.to_string(),
        stderr: "the command printed unexpected output".to_string(),
    }
}

pub(super) fn require_success(arguments: &[&str], output: &Output) -> Result<(), OcrScopeError> {
    if output.status.success() {
        return Ok(());
    }

    Err(OcrScopeError::Git {
        command: arguments.join(" "),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(super) fn git_text(git: &Git<'_>, arguments: &[&str]) -> Result<String, OcrScopeError> {
    let output = git.run(arguments)?;

    require_success(arguments, &output)?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn git_path(git: &Git<'_>, arguments: &[&str]) -> Result<PathBuf, OcrScopeError> {
    let output = git.run(arguments)?;

    require_success(arguments, &output)?;

    let path = output.stdout.strip_suffix(b"\n").unwrap_or(&output.stdout);

    Ok(path_from_bytes(path))
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    PathBuf::from(OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

pub(super) fn git_text_with_environment(
    git: &Git<'_>,
    arguments: &[&str],
    environment: &[(&str, &str)],
) -> Result<String, OcrScopeError> {
    let output = git.run_with_environment(arguments, environment)?;

    require_success(arguments, &output)?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn path_text(bytes: &[u8]) -> Result<String, OcrScopeError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| OcrScopeError::NotText {
        path: String::from_utf8_lossy(bytes).into_owned(),
    })
}

pub(super) struct TreeEntry {
    pub(super) mode: String,
    pub(super) kind: String,
    pub(super) object: String,
}

pub(super) fn tree_entries(
    git: &Git<'_>,
    revision: &str,
) -> Result<BTreeMap<String, TreeEntry>, OcrScopeError> {
    let arguments = ["ls-tree", "-r", "-z", "--full-tree", revision];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)?;

    parse_tree(&output.stdout)
}

fn parse_tree(bytes: &[u8]) -> Result<BTreeMap<String, TreeEntry>, OcrScopeError> {
    let fields: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect();
    let mut entries = BTreeMap::new();

    for field in fields {
        let tab = field
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| malformed("ls-tree"))?;
        let metadata = String::from_utf8_lossy(&field[..tab]);
        let mut parts = metadata.split(' ');
        let mode = parts.next().ok_or_else(|| malformed("ls-tree"))?;
        let kind = parts.next().ok_or_else(|| malformed("ls-tree"))?;
        let object = parts.next().ok_or_else(|| malformed("ls-tree"))?;
        let path = path_text(&field[tab + 1..])?;

        entries.insert(
            path,
            TreeEntry {
                mode: mode.to_string(),
                kind: kind.to_string(),
                object: object.to_string(),
            },
        );
    }

    Ok(entries)
}

pub(super) fn blob(git: &Git<'_>, object: &str) -> Result<Vec<u8>, OcrScopeError> {
    let arguments = ["cat-file", "blob", object];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)?;

    Ok(output.stdout)
}

pub(super) fn hash_object(git: &Git<'_>, content: &[u8]) -> Result<String, OcrScopeError> {
    let arguments = ["hash-object", "-w", "--stdin"];
    let output = git.run_with_input(&arguments, content)?;

    require_success(&arguments, &output)?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) fn commit_tree(git: &Git<'_>, tree: &str) -> Result<String, OcrScopeError> {
    let arguments = ["commit-tree", tree, "-m", "stomme ocr-scope base"];
    let environment = [
        ("GIT_AUTHOR_NAME", "stomme"),
        ("GIT_AUTHOR_EMAIL", "stomme@localhost"),
        ("GIT_COMMITTER_NAME", "stomme"),
        ("GIT_COMMITTER_EMAIL", "stomme@localhost"),
        ("GIT_AUTHOR_DATE", "@0 +0000"),
        ("GIT_COMMITTER_DATE", "@0 +0000"),
    ];

    git_text_with_environment(git, &arguments, &environment)
}

pub(super) fn graft(git: &Git<'_>, head: &str, commit: &str) -> Result<(), OcrScopeError> {
    let arguments = ["replace", "-f", "--graft", head, commit];
    let output = git.run(&arguments)?;

    require_success(&arguments, &output)
}

pub(super) fn content_lines(content: &[u8]) -> Vec<&[u8]> {
    if content.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<&[u8]> = content.split(|byte| *byte == b'\n').collect();

    if content.ends_with(b"\n") {
        lines.pop();
    }

    lines
}
