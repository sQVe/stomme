use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

pub(crate) struct TargetBranch {
    pub(crate) reference: String,
    pub(crate) read: fn(root: &Path, reference: &str) -> TargetRead,
    pub(crate) resolve: fn(root: &Path, reference: &str) -> Result<String, String>,
}

impl TargetBranch {
    pub(crate) fn git(reference: String) -> Self {
        Self {
            reference,
            read: read_git,
            resolve: resolve_expression,
        }
    }

    pub(crate) fn remote_tracking(reference: String) -> Self {
        Self {
            reference,
            read: read_remote_tracking,
            resolve: resolve_tracking,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_reader(reference: String, read: fn(&Path, &str) -> TargetRead) -> Self {
        Self {
            reference,
            read,
            resolve: resolve_expression,
        }
    }
}

pub(crate) fn default_reference(pull_request_base: Option<OsString>) -> anyhow::Result<String> {
    let Some(base) = pull_request_base else {
        return Ok("refs/remotes/origin/main".to_string());
    };

    if base.is_empty() {
        return Ok("refs/remotes/origin/main".to_string());
    }

    let name = base
        .into_string()
        .map_err(|_| anyhow::anyhow!("GITHUB_BASE_REF is not valid UTF-8"))?;

    Ok(format!("refs/remotes/origin/{name}"))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TargetRead {
    Unavailable(String),
    NoConfig,
    Config(String),
}

fn read_git(root: &Path, reference: &str) -> TargetRead {
    match resolve_expression(root, reference) {
        Ok(revision) => read_config(root, &revision, reference),
        Err(reason) => TargetRead::Unavailable(reason),
    }
}

fn read_remote_tracking(root: &Path, reference: &str) -> TargetRead {
    match resolve_tracking(root, reference) {
        Ok(revision) => read_config(root, &revision, reference),
        Err(reason) => TargetRead::Unavailable(reason),
    }
}

fn resolve_expression(root: &Path, reference: &str) -> Result<String, String> {
    resolve_commit(root, reference, reference)
}

fn resolve_tracking(root: &Path, reference: &str) -> Result<String, String> {
    let resolved = run_git(root, &["show-ref", "--verify", "--hash", reference]);

    let object_identifier = match resolved {
        Err(error) => return Err(format!("could not run `git`: {error}")),
        Ok(output) if !output.status.success() => return Err(unresolved(reference)),
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
    };

    resolve_commit(root, object_identifier.trim(), reference)
}

fn resolve_commit(root: &Path, revision: &str, reference: &str) -> Result<String, String> {
    let commit_argument = format!("{revision}^{{commit}}");
    let resolved = run_git(
        root,
        &["rev-parse", "--verify", "--quiet", &commit_argument],
    );

    match resolved {
        Err(error) => Err(format!("could not run `git`: {error}")),
        Ok(output) if !output.status.success() => Err(unresolved(reference)),
        Ok(output) => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
    }
}

fn read_config(root: &Path, revision: &str, reference: &str) -> TargetRead {
    let object = format!("{revision}:stomme.toml");
    let present = run_git(root, &["cat-file", "-e", &object]);

    match present {
        Err(error) => return TargetRead::Unavailable(format!("could not run `git`: {error}")),
        Ok(output) if !output.status.success() => return TargetRead::NoConfig,
        Ok(_) => {}
    }

    let shown = run_git(root, &["show", &object]);

    match shown {
        Err(error) => TargetRead::Unavailable(format!("could not run `git`: {error}")),
        Ok(output) if !output.status.success() => {
            TargetRead::Unavailable(format!("cannot read `stomme.toml` from `{reference}`"))
        }
        Ok(output) => TargetRead::Config(String::from_utf8_lossy(&output.stdout).into_owned()),
    }
}

fn unresolved(reference: &str) -> String {
    format!("the ref `{reference}` does not resolve")
}

pub(crate) fn run_git(root: &Path, arguments: &[&str]) -> io::Result<Output> {
    Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
}

#[cfg(test)]
mod tests;
