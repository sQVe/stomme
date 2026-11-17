use super::{Problem, is_pascal_case, problem};
use crate::check::source_files::{SourceFile, Tree};
use crate::rule::Rule;

pub(super) fn check(tree: &Tree) -> Vec<Problem> {
    let mut problems = Vec::new();

    for file in &tree.files {
        if !file.is_tsx() {
            continue;
        }

        if is_test_file(file) {
            problems.extend(test_placement(file, tree));
        } else {
            problems.extend(component_file(file));
        }
    }

    problems
}

fn component_file(file: &SourceFile) -> Option<Problem> {
    if is_pascal_case(&file.stem) {
        return None;
    }

    Some(problem(
        file,
        Rule::ReactComponentFile,
        format!("names the component file `{}`; use PascalCase", file.name),
    ))
}

fn test_placement(file: &SourceFile, tree: &Tree) -> Option<Problem> {
    let base = file.name.strip_suffix(".test.tsx")?;

    if has_component(tree, file, base) {
        return None;
    }

    Some(problem(
        file,
        Rule::ReactTestPlacement,
        format!(
            "puts the test `{}` beside no component; name it `<Component>.test.tsx` beside `<Component>.tsx`",
            file.name
        ),
    ))
}

fn has_component(tree: &Tree, file: &SourceFile, base: &str) -> bool {
    tree.files.iter().any(|sibling| {
        sibling.folder == file.folder
            && sibling.is_tsx()
            && !is_test_file(sibling)
            && is_component_stem(&sibling.stem, base)
    })
}

fn is_component_stem(stem: &str, base: &str) -> bool {
    if stem == base {
        return true;
    }

    let Some(rest) = base.strip_prefix(stem) else {
        return false;
    };
    let mut characters = rest.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    first.is_ascii_uppercase()
}

fn is_test_file(file: &SourceFile) -> bool {
    file.name.ends_with(".test.tsx")
}

#[cfg(test)]
mod tests;
