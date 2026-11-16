use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::cargo_manifest::{
    Dependencies, Dependency, ROOT_MANIFEST, RootManifest, Workspace, read_manifest,
};
use super::{Problem, Rule};

const MEMBER_MISSING: &str = "is missing; the workspace makes this folder a member";

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct MemberManifest {
    #[serde(default)]
    lints: MemberLints,
    #[serde(default)]
    dependencies: Dependencies,
    #[serde(default)]
    dev_dependencies: Dependencies,
    #[serde(default)]
    build_dependencies: Dependencies,
    #[serde(default)]
    target: BTreeMap<String, TargetDependencies>,
}

#[derive(Default, Deserialize)]
struct MemberLints {
    #[serde(default)]
    workspace: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct TargetDependencies {
    #[serde(default)]
    dependencies: Dependencies,
    #[serde(default)]
    dev_dependencies: Dependencies,
    #[serde(default)]
    build_dependencies: Dependencies,
}

type MemberRead = Result<MemberManifest, Problem>;

pub(super) fn check(root: &Path, manifest: &RootManifest) -> anyhow::Result<Vec<Problem>> {
    let workspace = &manifest.workspace;
    let mut problems = Vec::new();
    let mut listed = Vec::new();

    if manifest.has_package() {
        listed.push(String::new());
    }

    for pattern in &workspace.members {
        match patterns::expand(root, pattern)? {
            Ok(paths) => listed.extend(paths),
            Err(problem) => problems.push(problem),
        }
    }

    let members = read_members(root, workspace, listed)?;

    for (member, manifest) in members {
        problems.extend(inheritance_problem(member, manifest));
    }

    Ok(problems)
}

fn read_members(
    root: &Path,
    workspace: &Workspace,
    listed: Vec<String>,
) -> anyhow::Result<BTreeMap<String, MemberRead>> {
    let excluded: Vec<String> = workspace
        .exclude
        .iter()
        .map(|path| segments(path).join("/"))
        .collect();
    let mut members = BTreeMap::new();
    let mut pending = listed;

    while let Some(member) = pending.pop() {
        if members.contains_key(&member) || is_excluded(&member, &excluded) {
            continue;
        }

        let manifest: MemberRead =
            read_manifest(root, &join(&member, ROOT_MANIFEST), MEMBER_MISSING)?;

        if let Ok(manifest) = &manifest {
            pending.extend(path_dependencies(root, &member, manifest, workspace));
        }

        members.insert(member, manifest);
    }

    Ok(members)
}

fn path_dependencies(
    root: &Path,
    member: &str,
    manifest: &MemberManifest,
    workspace: &Workspace,
) -> Vec<String> {
    let mut paths = Vec::new();

    for (name, dependency) in manifest.dependency_tables().flatten() {
        let (base, path) = if dependency.inherits() {
            let inherited = workspace.dependencies.get(name);

            ("", inherited.and_then(Dependency::path))
        } else {
            (member, dependency.path())
        };

        paths.extend(path.and_then(|path| resolve_inside_root(root, base, path)));
    }

    paths
}

fn resolve_inside_root(root: &Path, base: &str, path: &str) -> Option<String> {
    let (base, path) = if Path::new(path).is_absolute() {
        ("", Path::new(path).strip_prefix(root).ok()?.to_str()?)
    } else {
        (base, path)
    };
    let mut resolved = segments(base);

    for segment in segments(path) {
        if segment == ".." {
            resolved.pop()?;
        } else {
            resolved.push(segment);
        }
    }

    Some(resolved.join("/"))
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect()
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

fn is_excluded(member: &str, excluded: &[String]) -> bool {
    excluded
        .iter()
        .any(|path| member == path || member.starts_with(&format!("{path}/")))
}

fn inheritance_problem(member: String, manifest: MemberRead) -> Option<Problem> {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(problem) => return Some(problem),
    };

    if manifest.lints.workspace {
        return None;
    }

    Some(Problem {
        path: join(&member, ROOT_MANIFEST),
        rule: Rule::RustLintInheritance,
        message:
            "lacks `[lints] workspace = true`; every workspace member inherits the shared lints"
                .to_string(),
        key: None,
    })
}

impl MemberManifest {
    fn dependency_tables(&self) -> impl Iterator<Item = &Dependencies> {
        let targets = self.target.values().flat_map(|target| {
            [
                &target.dependencies,
                &target.dev_dependencies,
                &target.build_dependencies,
            ]
        });

        [
            &self.dependencies,
            &self.dev_dependencies,
            &self.build_dependencies,
        ]
        .into_iter()
        .chain(targets)
    }
}

mod patterns;

#[cfg(test)]
mod tests;
