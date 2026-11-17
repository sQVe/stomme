use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use serde::Deserialize;

use super::super::Problem;
use super::super::cargo_manifest::{config_problem, read_toml_file};
#[cfg(test)]
use super::super::describe_toml_error;

const LOCK_FILE: &str = "Cargo.lock";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Package {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) source: Option<String>,
}

#[derive(Deserialize)]
struct LockFile {
    #[serde(default)]
    package: Vec<RawPackage>,
}

#[derive(Deserialize)]
struct RawPackage {
    name: String,
    version: String,
    source: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

#[derive(Debug)]
pub(super) struct Lock {
    packages: Vec<Package>,
    graph: BTreeMap<Package, Vec<Package>>,
}

pub(super) fn read(root: &Path) -> anyhow::Result<Result<Lock, Problem>> {
    let file = match read_toml_file::<LockFile>(root, LOCK_FILE)? {
        Some(Ok(file)) => file,
        Some(Err(reason)) => {
            return Ok(Err(problem(format!("is not a valid lock file: {reason}"))));
        }
        None => {
            return Ok(Err(problem(
                "is missing; a package boundary check needs the resolved dependency graph"
                    .to_string(),
            )));
        }
    };

    match Lock::new(file) {
        Ok(lock) => Ok(Ok(lock)),
        Err(reason) => Ok(Err(problem(reason))),
    }
}

#[cfg(test)]
pub(super) fn parse(text: &str) -> Result<Lock, String> {
    let file: LockFile = toml::from_str(text).map_err(|error| describe_toml_error(&error, text))?;

    Lock::new(file)
}

impl Lock {
    fn new(file: LockFile) -> Result<Self, String> {
        let mut packages = Vec::new();
        let mut by_name: BTreeMap<String, Vec<Package>> = BTreeMap::new();
        let mut raw_dependencies = Vec::new();

        for raw in file.package {
            let RawPackage {
                name,
                version,
                source,
                dependencies,
            } = raw;
            let package = Package {
                name,
                version,
                source,
            };

            by_name
                .entry(package.name.clone())
                .or_default()
                .push(package.clone());
            raw_dependencies.push((package.clone(), dependencies));
            packages.push(package);
        }

        let mut graph = BTreeMap::new();

        for (package, dependencies) in raw_dependencies {
            let resolved = dependencies
                .iter()
                .map(|entry| resolve(&by_name, entry))
                .collect::<Result<Vec<_>, _>>()?;

            graph.insert(package, resolved);
        }

        Ok(Self { packages, graph })
    }

    pub(super) fn role_package(&self, name: &str) -> Option<Package> {
        self.packages
            .iter()
            .find(|package| package.name == name && package.source.is_none())
            .cloned()
    }

    pub(super) fn dependencies(&self, package: &Package) -> &[Package] {
        self.graph.get(package).map(Vec::as_slice).unwrap_or(&[])
    }

    pub(super) fn find_path(
        &self,
        start: &Package,
        forbidden: impl Fn(&str) -> bool,
    ) -> Option<Vec<Package>> {
        let mut previous: BTreeMap<Package, Package> = BTreeMap::new();
        let mut visited: BTreeSet<Package> = BTreeSet::from([start.clone()]);
        let mut queue = VecDeque::from([start.clone()]);

        while let Some(package) = queue.pop_front() {
            if forbidden(&package.name) {
                return Some(path_to(&previous, start, &package));
            }

            for dependency in self.dependencies(&package) {
                if visited.insert(dependency.clone()) {
                    previous.insert(dependency.clone(), package.clone());
                    queue.push_back(dependency.clone());
                }
            }
        }

        None
    }
}

fn resolve(by_name: &BTreeMap<String, Vec<Package>>, entry: &str) -> Result<Package, String> {
    let mut parts = entry.split_whitespace();
    let name = parts
        .next()
        .ok_or_else(|| format!("has the empty dependency entry `{entry}`"))?;
    let version = parts.next();
    let source = parts
        .next()
        .map(|part| part.trim_start_matches('(').trim_end_matches(')'));

    if parts.next().is_some() {
        return Err(format!(
            "has the dependency entry `{entry}`, which is not `name`, `name version`, or `name version (source)`"
        ));
    }

    let candidates = by_name.get(name).map(Vec::as_slice).unwrap_or(&[]);

    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }

    let mut matches: Vec<&Package> = candidates.iter().collect();

    if let Some(version) = version {
        matches.retain(|package| package.version == version);
    }

    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }

    if let Some(source) = source {
        matches.retain(|package| package.source.as_deref() == Some(source));
    }

    match matches.as_slice() {
        [package] => Ok((*package).clone()),
        [] => Err(format!(
            "has the dependency `{entry}`, which names no lock package"
        )),
        _ => Err(format!(
            "has the dependency `{entry}`, which matches more than one lock package"
        )),
    }
}

fn path_to(
    previous: &BTreeMap<Package, Package>,
    start: &Package,
    target: &Package,
) -> Vec<Package> {
    let mut path = vec![target.clone()];
    let mut current = target;

    while current != start {
        let Some(parent) = previous.get(current) else {
            break;
        };

        current = parent;
        path.push(current.clone());
    }

    path.reverse();
    path
}

fn problem(message: String) -> Problem {
    config_problem(LOCK_FILE.to_string(), message)
}

#[cfg(test)]
mod tests;
