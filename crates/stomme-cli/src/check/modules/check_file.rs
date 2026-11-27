use std::collections::BTreeSet;

use super::resolution::Resolver;
use super::use_paths::{Namespace, UsePath, paths};
use super::{Module, module_path};
use crate::check::Problem;
use crate::check::source_files::{SourceFile, Tree};
use crate::rule::Rule;

pub(super) fn check_file(
    resolver: &Resolver,
    module: &Module,
    file: &SourceFile,
    source: &str,
    file_is_test: bool,
) -> Vec<Problem> {
    let file_path = module_path(&file.path, &module.src_folder);
    let table = resolver.bindings(module, &file_path);
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();

    for use_path in paths(source, module.edition, &table) {
        let mut current = file_path.clone();
        current.extend(use_path.inline_modules.iter().cloned());
        let in_test_module = file_is_test || use_path.in_test_module;
        let Some(resolved) = resolve(&use_path, &current) else {
            continue;
        };
        let found = classify(resolver, module, file, &resolved, &use_path, in_test_module);

        for problem in found {
            let key = (problem.rule.id(), problem.key.clone());

            if seen.insert(key) {
                problems.push(problem);
            }
        }
    }

    problems
}

fn resolve(use_path: &UsePath, file_path: &[String]) -> Option<Vec<String>> {
    let mut current = file_path.to_vec();
    let first = use_path.segments.first().map(String::as_str);

    if first == Some("super") {
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
    } else if first == Some("self") {
        current.extend(use_path.segments[1..].iter().cloned());
    } else {
        current.clear();
        current.extend(use_path.segments[1..].iter().cloned());
    }

    Some(current)
}

fn classify(
    resolver: &Resolver,
    module: &Module,
    file: &SourceFile,
    resolved: &[String],
    use_path: &UsePath,
    in_test_module: bool,
) -> Vec<Problem> {
    let parent = value_path_parent(resolver, module, resolved, use_path);
    let resolved = parent.as_deref().unwrap_or(resolved);
    let Some(found) = longest_prefix(resolver.modules, module, resolved) else {
        let unlisted = classify_unlisted(resolver.tree, module, file, resolved, use_path);

        return unlisted.into_iter().collect();
    };

    if found.name == module.name {
        return Vec::new();
    }

    if !module.imports.contains(&found.name) {
        let message = format!(
            "line {}: module `{}` uses module `{}`, which `[modules.{}] imports` does not list",
            use_path.line, module.name, found.name, module.name
        );
        let key = format!("{}:{}", module.name, found.name);

        return vec![problem(file, Rule::ModuleImports, message, key)];
    }

    if in_test_module {
        return Vec::new();
    }

    private_file_problems(resolver, module, found, file, resolved, use_path)
}

fn value_path_parent(
    resolver: &Resolver,
    module: &Module,
    resolved: &[String],
    use_path: &UsePath,
) -> Option<Vec<String>> {
    if use_path.namespace != Namespace::Value {
        return None;
    }

    let is_row = resolver
        .modules
        .iter()
        .any(|row| row.src_folder == module.src_folder && row.path.as_slice() == resolved);

    if !is_row {
        return None;
    }

    let (last, parent_path) = resolved.split_last()?;
    let parent = longest_prefix(resolver.modules, module, parent_path)?;

    if !resolver.declares_value_item(parent, parent_path, last) {
        return None;
    }

    Some(parent_path.to_vec())
}

fn private_file_problems(
    resolver: &Resolver,
    module: &Module,
    found: &Module,
    file: &SourceFile,
    resolved: &[String],
    use_path: &UsePath,
) -> Vec<Problem> {
    let reached = resolver.reached_files(found, &module.path, resolved, use_path.namespace);

    reached
        .into_iter()
        .filter(|used_file| !found.public.contains(used_file))
        .map(|used_file| {
            let message = format!(
                "line {}: module `{}` uses `{}`, which `[modules.{}] public` does not list",
                use_path.line, module.name, used_file, found.name
            );
            let key = format!("{}:{}", module.name, used_file);

            problem(file, Rule::ModulePrivateFile, message, key)
        })
        .collect()
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
