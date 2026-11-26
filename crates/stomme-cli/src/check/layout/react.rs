use super::typescript;
use super::{Problem, is_pascal_case, problem};
use crate::check::source_files::{SourceFile, Tree};
use crate::rule::Rule;

pub(super) fn check(tree: &Tree) -> Vec<Problem> {
    let mut problems = Vec::new();

    for file in &tree.files {
        if !file.is_tsx() {
            continue;
        }

        if file.name.ends_with(".test.tsx") {
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

// Placement follows the TypeScript layer's owner rule, so both layers give one answer for a tree.
fn test_placement(file: &SourceFile, tree: &Tree) -> Option<Problem> {
    let base = file.name.strip_suffix(".test.tsx")?;

    if typescript::has_owner(tree, file, base) {
        return None;
    }

    Some(problem(
        file,
        Rule::ReactTestPlacement,
        format!(
            "puts the test `{}` beside no owner; name it `<owner>.test.tsx` beside `<owner>.ts` or `<owner>.tsx`",
            file.name
        ),
    ))
}

#[cfg(test)]
mod tests;
