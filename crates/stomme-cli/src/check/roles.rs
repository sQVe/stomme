use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::Problem;
use crate::config::CONFIG_FILE;
use crate::rule::Rule;

const ROLES: &[&str] = &["core", "ui", "kit", "app", "cli"];
const ROLE_NAMES: &str = "`core`, `ui`, `kit`, `app`, and `cli`";

pub(super) fn check(root: &Path, roles: &BTreeMap<String, String>) -> Vec<Problem> {
    roles
        .iter()
        .filter_map(|(name, path)| role_problem(root, name, path))
        .collect()
}

fn role_problem(root: &Path, name: &str, path: &str) -> Option<Problem> {
    if !ROLES.contains(&name) {
        return Some(Problem {
            path: CONFIG_FILE.to_string(),
            rule: Rule::RoleName,
            message: format!("names the unknown role `{name}`; the roles are {ROLE_NAMES}"),
            key: None,
        });
    }

    if let Some(problem) = path_problem(root, name, path) {
        return Some(problem);
    }

    folder_problem(name, path)
}

fn path_problem(root: &Path, name: &str, path: &str) -> Option<Problem> {
    let message = if path.is_empty() {
        format!("gives role `{name}` an empty path; use an existing folder inside the project")
    } else if normalize(path).is_empty() {
        format!(
            "gives role `{name}` the path `{path}`, which is the project root; use a folder inside the project"
        )
    } else if Path::new(path).is_absolute() {
        format!("gives role `{name}` the absolute path `{path}`; use a path inside the project")
    } else if path.split('/').any(|part| part == "..") {
        format!(
            "gives role `{name}` the path `{path}` with a `..` part; use a path inside the project"
        )
    } else if !root.join(path).is_dir() {
        format!("gives role `{name}` the path `{path}`, which is not an existing folder")
    } else if resolves_outside(root, path) {
        format!("gives role `{name}` the path `{path}`, which resolves outside the project")
    } else {
        return None;
    };

    Some(Problem {
        path: CONFIG_FILE.to_string(),
        rule: Rule::RolePath,
        message,
        key: None,
    })
}

fn resolves_outside(root: &Path, path: &str) -> bool {
    let Ok(root) = fs::canonicalize(root) else {
        return false;
    };
    let Ok(target) = fs::canonicalize(root.join(path)) else {
        return false;
    };

    !target.starts_with(&root)
}

fn folder_problem(name: &str, path: &str) -> Option<Problem> {
    let folder = Path::new(path).file_name()?.to_string_lossy();
    let accepted = folder == name || folder.ends_with(&format!("-{name}"));

    if accepted {
        return None;
    }

    Some(Problem {
        path: path.to_string(),
        rule: Rule::RoleFolder,
        message: format!(
            "gives role `{name}` the folder `{path}`; name the folder `{name}` or end it with `-{name}`"
        ),
        key: Some(format!("{name}:{folder}")),
    })
}

fn normalize(path: &str) -> String {
    path.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests;
