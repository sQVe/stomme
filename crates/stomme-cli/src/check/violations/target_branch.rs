use std::io;
use std::path::Path;
use std::process::{Command, Output};

pub(crate) struct TargetBranch {
    pub(crate) reference: String,
    pub(crate) read: fn(root: &Path, reference: &str) -> TargetRead,
}

impl TargetBranch {
    pub(crate) fn git(reference: String) -> Self {
        Self {
            reference,
            read: read_git,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TargetRead {
    Unavailable(String),
    NoConfig,
    Config(String),
}

fn read_git(root: &Path, reference: &str) -> TargetRead {
    let commit_argument = format!("{reference}^{{commit}}");
    let resolved = run_git(
        root,
        &["rev-parse", "--verify", "--quiet", &commit_argument],
    );

    match resolved {
        Err(error) => return TargetRead::Unavailable(format!("could not run `git`: {error}")),
        Ok(output) if !output.status.success() => {
            return TargetRead::Unavailable(format!("the ref `{reference}` does not resolve"));
        }
        Ok(_) => {}
    }

    let object = format!("{reference}:stomme.toml");
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
            TargetRead::Unavailable(format!("could not read `{object}`"))
        }
        Ok(output) => TargetRead::Config(String::from_utf8_lossy(&output.stdout).into_owned()),
    }
}

fn run_git(root: &Path, arguments: &[&str]) -> io::Result<Output> {
    Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
}

#[cfg(test)]
mod tests;
