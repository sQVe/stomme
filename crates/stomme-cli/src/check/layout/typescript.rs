use std::collections::{BTreeMap, BTreeSet};

use super::{
    Problem, catch_all_folders, is_camel_case, is_catch_all, is_pascal_case, join, problem,
};
use crate::check::source_files::{SourceFile, Tree};
use crate::layer::Refinement;
use crate::rule::Rule;

pub(super) fn check(
    tree: &Tree,
    refinements: &BTreeSet<Refinement>,
) -> anyhow::Result<Vec<Problem>> {
    let skip_tsx = refinements.contains(&Refinement::TsxComponents);
    let mut problems = Vec::new();

    for file in &tree.files {
        problems.extend(file_case(tree, file, skip_tsx)?);
        problems.extend(catch_all_file(file));
        problems.extend(index_file(file));
        problems.extend(test_name(file, tree, skip_tsx));
    }

    problems.extend(catch_all_folders(tree, Rule::TypeScriptCatchAllName));
    problems.extend(owner_folder(tree));

    Ok(problems)
}

fn file_case(tree: &Tree, file: &SourceFile, skip_tsx: bool) -> anyhow::Result<Option<Problem>> {
    if skip_tsx && file.is_tsx() {
        return Ok(None);
    }

    if is_camel_case(&file.stem) {
        return Ok(None);
    }

    if is_pascal_case(&file.stem) && exports_class(&tree.read(file)?, &file.stem) {
        return Ok(None);
    }

    if let Some(base) = test_base(file)
        && has_class_owner(tree, file, base)?
    {
        return Ok(None);
    }

    Ok(Some(problem(
        file,
        Rule::TypeScriptFileCase,
        format!(
            "names the file `{}`; use camelCase, or PascalCase for a file that exports one class",
            file.name
        ),
    )))
}

fn is_production_sibling(file: &SourceFile, sibling: &SourceFile, extension: &str) -> bool {
    sibling.folder == file.folder && !is_test_file(sibling) && sibling.name.ends_with(extension)
}

fn has_class_owner(tree: &Tree, file: &SourceFile, base: &str) -> anyhow::Result<bool> {
    if !is_pascal_case(base) {
        return Ok(false);
    }

    let extension = if file.is_tsx() { ".tsx" } else { ".ts" };
    let owners = tree.files.iter().filter(|sibling| {
        is_production_sibling(file, sibling, extension) && is_owner_stem(base, &sibling.stem)
    });

    for owner in owners {
        if exports_class(&tree.read(owner)?, &owner.stem) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn exports_class(text: &str, stem: &str) -> bool {
    text.contains(&format!("export class {stem}"))
        || text.contains(&format!("export default class {stem}"))
}

fn catch_all_file(file: &SourceFile) -> Option<Problem> {
    if !is_catch_all(&file.stem) {
        return None;
    }

    Some(problem(
        file,
        Rule::TypeScriptCatchAllName,
        format!(
            "names the file `{}` after a catch-all; name it after its owner",
            file.name
        ),
    ))
}

fn index_file(file: &SourceFile) -> Option<Problem> {
    if file.stem != "index" {
        return None;
    }

    Some(problem(
        file,
        Rule::NoIndexFile,
        format!(
            "names the file `{}`; source folders hold no index files",
            file.name
        ),
    ))
}

fn test_name(file: &SourceFile, tree: &Tree, skip_tsx: bool) -> Option<Problem> {
    if skip_tsx && file.is_tsx() {
        return None;
    }

    if !file.path.split('/').any(|part| part == "src") {
        return None;
    }

    if file.path.split('/').any(|part| part == "__tests__") {
        return Some(problem(
            file,
            Rule::TestFileName,
            format!(
                "puts `{}` in a `__tests__` folder; put a unit test beside its owner",
                file.name
            ),
        ));
    }

    let base = test_base(file)?;

    if has_owner(tree, file, base) {
        return None;
    }

    Some(problem(
        file,
        Rule::TestFileName,
        format!(
            "names the test `{}` after no sibling module; name it after its owner",
            file.name
        ),
    ))
}

fn test_base(file: &SourceFile) -> Option<&str> {
    if file.name.ends_with(".test.ts") {
        file.name.strip_suffix(".test.ts")
    } else if file.name.ends_with(".test.tsx") {
        file.name.strip_suffix(".test.tsx")
    } else {
        None
    }
}

fn has_owner(tree: &Tree, file: &SourceFile, base: &str) -> bool {
    let extension = if file.is_tsx() { ".tsx" } else { ".ts" };

    tree.files.iter().any(|sibling| {
        sibling.folder == file.folder
            && !is_test_file(sibling)
            && sibling.name.ends_with(extension)
            && is_owner_stem(base, &sibling.stem)
    })
}

fn is_owner_stem(stem: &str, base: &str) -> bool {
    if stem == base {
        return true;
    }

    let Some(rest) = stem.strip_prefix(base) else {
        return false;
    };
    let mut characters = rest.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    first.is_ascii_uppercase() && characters.next().is_some()
}

fn is_test_file(file: &SourceFile) -> bool {
    file.name.ends_with(".test.ts")
        || file.name.ends_with(".test.tsx")
        || file.path.split('/').any(|part| part == "__tests__")
}

fn owner_folder(tree: &Tree) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut by_folder: BTreeMap<&str, Vec<&SourceFile>> = BTreeMap::new();

    for file in &tree.files {
        if file.is_ts() && !is_test_file(file) {
            by_folder
                .entry(file.folder.as_str())
                .or_default()
                .push(file);
        }
    }

    for (folder, files) in &by_folder {
        let stems: BTreeSet<&str> = files.iter().map(|file| file.stem.as_str()).collect();
        let mut owners = stems.clone();
        owners.extend(child_folders(tree, folder));
        let folder_name = folder.rsplit('/').next().unwrap_or(folder);

        for file in files {
            let Some(owner) = longest_owner(&owners, &file.stem) else {
                continue;
            };

            if folder_name == owner {
                continue;
            }

            let path = if stems.contains(owner) {
                join(folder, &format!("{owner}.ts"))
            } else {
                let base_folder = join(folder, owner);

                if !tree.folders.contains(&base_folder) {
                    continue;
                }

                base_folder
            };

            problems.push(Problem {
                path,
                rule: Rule::TypeScriptOwnerFolder,
                message: format!(
                    "puts `{}` outside the owner `{owner}`; move it into `{owner}/`",
                    file.name
                ),
                key: Some(file.name.clone()),
            });
        }
    }

    problems
}

fn child_folders<'a>(tree: &'a Tree, folder: &str) -> BTreeSet<&'a str> {
    tree.folders
        .iter()
        .filter_map(|candidate| {
            if folder.is_empty() {
                return (!candidate.is_empty() && !candidate.contains('/'))
                    .then_some(candidate.as_str());
            }

            let (parent, name) = candidate.rsplit_once('/')?;

            (parent == folder).then_some(name)
        })
        .collect()
}

fn longest_owner<'a>(owners: &BTreeSet<&'a str>, stem: &str) -> Option<&'a str> {
    owners
        .iter()
        .filter(|owner| is_family_stem(stem, owner))
        .max_by_key(|owner| owner.len())
        .copied()
}

fn is_family_stem(stem: &str, owner: &str) -> bool {
    let Some(rest) = stem.strip_prefix(owner) else {
        return false;
    };
    let mut characters = rest.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    first.is_ascii_uppercase() && characters.next().is_some()
}

#[cfg(test)]
mod tests;
