mod use_paths;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::Problem;
use super::source_files::{self, SourceFile, Tree};
use crate::config::ModuleRow;
use crate::layer::Language;
use crate::rule::Rule;
use use_paths::{UsePath, paths};

struct Module {
    name: String,
    entry: String,
    path: Vec<String>,
    src_folder: String,
    owner_folder: String,
    public: BTreeSet<String>,
    imports: BTreeSet<String>,
}

pub(super) fn check(
    root: &Path,
    modules: &BTreeMap<String, ModuleRow>,
) -> anyhow::Result<Vec<Problem>> {
    if modules.is_empty() {
        return Ok(Vec::new());
    }

    let tree = source_files::walk(root, Language::Rust)?;
    let modules = read_modules(modules);
    let src_folders: BTreeSet<String> = modules
        .iter()
        .map(|module| module.src_folder.clone())
        .collect();
    let mut problems = Vec::new();

    for file in &tree.files {
        let owner = owner_module(&modules, &file.path);

        let Some(owner) = owner else {
            if let Some(src_folder) = src_folder_for_file(&src_folders, &file.path)
                && !is_crate_root(&file.path, &src_folder)
                && !is_under_bin(&file.path, &src_folder)
            {
                problems.push(module_listed(file));
            }

            continue;
        };

        if is_crate_root(&file.path, &owner.src_folder)
            || is_under_bin(&file.path, &owner.src_folder)
        {
            continue;
        }

        let source = tree.read(file)?;
        problems.extend(check_file(&tree, owner, file, &source, &modules));
    }

    Ok(problems)
}

fn read_modules(rows: &BTreeMap<String, ModuleRow>) -> Vec<Module> {
    rows.iter()
        .map(|(name, row)| {
            let src_folder = src_folder(&row.entry).expect("config checked the `src` part");
            let path = module_path(&row.entry, &src_folder);
            let owner_folder = row
                .entry
                .strip_suffix(".rs")
                .unwrap_or(&row.entry)
                .to_string();

            Module {
                name: name.clone(),
                entry: row.entry.clone(),
                path,
                src_folder,
                owner_folder,
                public: row.public.iter().cloned().collect(),
                imports: row.imports.iter().cloned().collect(),
            }
        })
        .collect()
}

fn src_folder(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    let index = parts.iter().rposition(|part| *part == "src")?;

    Some(parts[..=index].join("/"))
}

fn module_path(path: &str, src_folder: &str) -> Vec<String> {
    let relative = path.strip_prefix(&format!("{src_folder}/")).unwrap_or(path);
    let without_extension = relative.strip_suffix(".rs").unwrap_or(relative);

    without_extension.split('/').map(str::to_string).collect()
}

fn src_folder_for_file(src_folders: &BTreeSet<String>, path: &str) -> Option<String> {
    src_folders
        .iter()
        .filter(|folder| path.starts_with(&format!("{folder}/")))
        .max_by_key(|folder| folder.len())
        .cloned()
}

fn is_crate_root(path: &str, src_folder: &str) -> bool {
    path == format!("{src_folder}/lib.rs") || path == format!("{src_folder}/main.rs")
}

fn is_under_bin(path: &str, src_folder: &str) -> bool {
    path.starts_with(&format!("{src_folder}/bin/"))
}

fn owner_module<'a>(modules: &'a [Module], path: &str) -> Option<&'a Module> {
    modules
        .iter()
        .filter(|module| owns(module, path))
        .max_by_key(|module| module.path.len())
}

fn owns(module: &Module, path: &str) -> bool {
    let folder_prefix = format!("{}/", module.owner_folder);

    path == module.entry || path.starts_with(&folder_prefix)
}

fn module_listed(file: &SourceFile) -> Problem {
    Problem {
        path: file.path.clone(),
        rule: Rule::ModuleListed,
        message: "belongs to no module in `[modules]`; add a row or move the file".to_string(),
        key: None,
    }
}

fn check_file(
    tree: &Tree,
    module: &Module,
    file: &SourceFile,
    source: &str,
    modules: &[Module],
) -> Vec<Problem> {
    let file_path = module_path(&file.path, &module.src_folder);
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();

    for use_path in paths(source) {
        let mut current = file_path.clone();
        current.extend(use_path.inline_modules.iter().cloned());
        let Some(resolved) = resolve(&use_path, &current) else {
            continue;
        };
        let Some(problem) = classify(tree, module, file, &resolved, &use_path, modules) else {
            continue;
        };
        let key = (problem.rule.id(), problem.key.clone());

        if seen.insert(key) {
            problems.push(problem);
        }
    }

    problems
}

fn resolve(use_path: &UsePath, file_path: &[String]) -> Option<Vec<String>> {
    let mut current = file_path.to_vec();
    let is_relative = use_path
        .segments
        .first()
        .is_some_and(|segment| segment == "super");

    if is_relative {
        let supers = use_path
            .segments
            .iter()
            .take_while(|segment| *segment == "super")
            .count();

        if supers > current.len() {
            return None;
        }

        current.truncate(current.len() - supers);
        current.extend(use_path.segments[supers..].iter().cloned());
    } else {
        current.clear();
        current.extend(use_path.segments[1..].iter().cloned());
    }

    Some(current)
}

fn classify(
    tree: &Tree,
    module: &Module,
    file: &SourceFile,
    resolved: &[String],
    use_path: &UsePath,
    modules: &[Module],
) -> Option<Problem> {
    let Some(found) = longest_prefix(modules, module, resolved) else {
        return classify_unlisted(tree, module, file, resolved, use_path);
    };

    if found.name == module.name {
        return None;
    }

    if !module.imports.contains(&found.name) {
        let message = format!(
            "line {}: module `{}` uses module `{}`, which `[modules.{}] imports` does not list",
            use_path.line, module.name, found.name, module.name
        );
        let key = format!("{}:{}", module.name, found.name);

        return Some(problem(file, Rule::ModuleImports, message, key));
    }

    let segment = resolved.get(found.path.len())?;
    let candidate_file = format!("{}/{}.rs", found.owner_folder, segment);
    let candidate_folder = format!("{}/{}", found.owner_folder, segment);
    let names_file = tree.files.iter().any(|file| file.path == candidate_file);
    let names_folder = tree.folders.contains(&candidate_folder);

    if (names_file || names_folder) && !found.public.contains(&candidate_file) {
        let message = format!(
            "line {}: module `{}` uses `{}`, which `[modules.{}] public` does not list",
            use_path.line, module.name, candidate_file, found.name
        );
        let key = format!("{}:{}", module.name, candidate_file);

        return Some(problem(file, Rule::ModulePrivateFile, message, key));
    }

    None
}

fn classify_unlisted(
    tree: &Tree,
    module: &Module,
    file: &SourceFile,
    resolved: &[String],
    use_path: &UsePath,
) -> Option<Problem> {
    let Some(first) = resolved.first() else {
        return Some(crate_root_itself(file, module, use_path));
    };

    if first == "*" {
        return Some(crate_root_path(file, module, use_path, "*"));
    }

    let names_file = tree
        .files
        .iter()
        .any(|file| file.path == format!("{}/{first}.rs", module.src_folder));
    let names_folder = tree
        .folders
        .contains(&format!("{}/{first}", module.src_folder));

    if !names_file && !names_folder {
        return Some(crate_root_path(file, module, use_path, first));
    }

    let message = format!(
        "line {}: module `{}` uses `{first}`, which no `[modules]` row covers",
        use_path.line, module.name
    );
    let key = format!("{}:{first}", module.name);

    Some(problem(file, Rule::ModuleImports, message, key))
}

fn crate_root_itself(file: &SourceFile, module: &Module, use_path: &UsePath) -> Problem {
    let message = format!(
        "line {}: module `{}` uses the crate root itself; use a module path",
        use_path.line, module.name
    );
    let key = format!("{}:crate", module.name);

    problem(file, Rule::CrateRootPath, message, key)
}

fn crate_root_path(file: &SourceFile, module: &Module, use_path: &UsePath, name: &str) -> Problem {
    let message = format!(
        "line {}: module `{}` uses the crate root path `{name}`; use a module path",
        use_path.line, module.name
    );
    let key = format!("{}:{name}", module.name);

    problem(file, Rule::CrateRootPath, message, key)
}

fn longest_prefix<'a>(
    modules: &'a [Module],
    current: &Module,
    resolved: &[String],
) -> Option<&'a Module> {
    modules
        .iter()
        .filter(|module| module.src_folder == current.src_folder)
        .filter(|module| resolved.starts_with(&module.path))
        .max_by_key(|module| module.path.len())
}

fn problem(file: &SourceFile, rule: Rule, message: String, key: String) -> Problem {
    Problem {
        path: file.path.clone(),
        rule,
        message,
        key: Some(key),
    }
}

#[cfg(test)]
mod tests;
