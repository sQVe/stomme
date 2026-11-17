use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::Problem;
use super::cargo_lock::{Lock, Package};
use super::cargo_manifest::{self, config_problem};
use crate::config::Selection;
use crate::rule::Rule;

const LOCK_FILE: &str = "Cargo.lock";
const GPUI_LAYER: &str = "gpui";
const GPUI_COMPONENT_LAYER: &str = "gpui-component";
const COMPONENT_PACKAGE: &str = "gpui-component";
const KIT_ROLE: &str = "kit";
const CORE_ROLES: &[&str] = &["core", "cli"];
const UI_ROLES: &[&str] = &["ui", "kit", "app"];

#[derive(Deserialize)]
struct Manifest {
    package: Option<PackageSection>,
}

#[derive(Deserialize)]
struct PackageSection {
    name: String,
}

struct RolePackage {
    role: String,
    path: String,
    name: String,
}

struct ResolvedRole {
    role: String,
    path: String,
    package: Package,
}

pub(super) fn check(
    root: &Path,
    selection: &Selection,
    lock: Option<&Lock>,
) -> anyhow::Result<Vec<Problem>> {
    let role_packages = match read_role_packages(root, selection)? {
        Ok(role_packages) => role_packages,
        Err(problem) => return Ok(vec![problem]),
    };
    let plain_core = selection.layers.contains(GPUI_LAYER)
        && role_packages
            .iter()
            .any(|role| CORE_ROLES.contains(&role.role.as_str()));
    let component_kit = selection.layers.contains(GPUI_COMPONENT_LAYER)
        && role_packages.iter().any(|role| role.role != KIT_ROLE);

    if !plain_core && !component_kit {
        return Ok(Vec::new());
    }

    let Some(lock) = lock else {
        return Ok(vec![config_problem(
            LOCK_FILE.to_string(),
            "is missing; a package boundary check needs the resolved dependency graph".to_string(),
        )]);
    };
    let roles = match resolve_role_packages(lock, &role_packages) {
        Ok(roles) => roles,
        Err(problem) => return Ok(vec![problem]),
    };
    let mut problems = Vec::new();

    if plain_core {
        problems.extend(plain_core_problems(lock, &roles));
    }

    if component_kit {
        problems.extend(component_kit_problems(lock, &roles));
    }

    Ok(problems)
}

fn read_role_packages(
    root: &Path,
    selection: &Selection,
) -> anyhow::Result<Result<Vec<RolePackage>, Problem>> {
    let mut packages = Vec::new();

    for (role, path) in &selection.roles {
        let manifest_path = format!("{path}/Cargo.toml");
        let manifest = match cargo_manifest::read_toml_file::<Manifest>(root, &manifest_path)? {
            Some(Ok(manifest)) => manifest,
            Some(Err(reason)) => {
                let message = format!("is not a valid manifest: {reason}");

                return Ok(Err(config_problem(manifest_path, message)));
            }
            None => continue,
        };
        let Some(package) = manifest.package else {
            continue;
        };

        packages.push(RolePackage {
            role: role.clone(),
            path: path.clone(),
            name: package.name,
        });
    }

    Ok(Ok(packages))
}

fn resolve_role_packages(
    lock: &Lock,
    role_packages: &[RolePackage],
) -> Result<Vec<ResolvedRole>, Problem> {
    let mut roles = Vec::new();

    for role in role_packages {
        let Some(package) = lock.role_package(&role.name) else {
            let message = format!(
                "has no package `{}` for role `{}`; the lock is out of date",
                role.name, role.role
            );

            return Err(config_problem(LOCK_FILE.to_string(), message));
        };

        roles.push(ResolvedRole {
            role: role.role.clone(),
            path: role.path.clone(),
            package,
        });
    }

    Ok(roles)
}

fn plain_core_problems(lock: &Lock, roles: &[ResolvedRole]) -> Vec<Problem> {
    let forbidden_roles: BTreeMap<&str, &str> = roles
        .iter()
        .filter(|role| UI_ROLES.contains(&role.role.as_str()))
        .map(|role| (role.package.name.as_str(), role.role.as_str()))
        .collect();
    let is_forbidden = |name: &str| {
        name == "gpui"
            || name.starts_with("gpui-")
            || name.starts_with("gpui_")
            || forbidden_roles.contains_key(name)
    };

    roles
        .iter()
        .filter(|role| CORE_ROLES.contains(&role.role.as_str()))
        .filter_map(|role| {
            let path = lock.find_path(&role.package, is_forbidden)?;
            let forbidden = path.last()?;
            let chain = path
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>()
                .join(" -> ");

            Some(Problem {
                path: role.path.clone(),
                rule: Rule::GpuiPlainCore,
                message: format!(
                    "role `{}` package `{}` depends on `{}` through {chain}; owner is layer `{GPUI_LAYER}`",
                    role.role, role.package.name, forbidden.name
                ),
                key: Some(format!("{}:{}", role.role, forbidden.name)),
            })
        })
        .collect()
}

fn component_kit_problems(lock: &Lock, roles: &[ResolvedRole]) -> Vec<Problem> {
    let kit = roles.iter().find(|role| role.role == KIT_ROLE);

    roles
        .iter()
        .filter(|role| role.role != KIT_ROLE)
        .filter(|role| {
            lock.dependencies(&role.package)
                .iter()
                .any(|dependency| dependency.name == COMPONENT_PACKAGE)
        })
        .map(|role| {
            let message = match kit {
                Some(kit) => format!(
                    "package `{}` lists `{COMPONENT_PACKAGE}` as a direct dependency; only the `kit` package `{}` may, under layer `{GPUI_COMPONENT_LAYER}`",
                    role.package.name, kit.package.name
                ),
                None => format!(
                    "package `{}` lists `{COMPONENT_PACKAGE}` as a direct dependency; add a `kit` role and import it there, under layer `{GPUI_COMPONENT_LAYER}`",
                    role.package.name
                ),
            };

            Problem {
                path: LOCK_FILE.to_string(),
                rule: Rule::GpuiComponentKit,
                message,
                key: Some(role.package.name.clone()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
