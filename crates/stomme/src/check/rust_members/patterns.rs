use std::fs;
use std::path::Path;

use anyhow::Context as _;

use super::{join, segments};
use crate::check::Problem;
use crate::check::cargo_manifest::{ROOT_MANIFEST, config_problem};

pub(super) fn expand(root: &Path, pattern: &str) -> anyhow::Result<Result<Vec<String>, Problem>> {
    let segments = segments(pattern);

    if segments.iter().any(|segment| is_unsupported_glob(segment)) {
        return Ok(Err(config_problem(
            ROOT_MANIFEST.to_string(),
            format!(
                "lists member `{pattern}`; stomme expands only literal paths and `*` inside one path segment"
            ),
        )));
    }

    let mut paths = vec![String::new()];

    for segment in segments {
        if segment.contains('*') {
            let mut matched = Vec::new();

            for parent in &paths {
                matched.extend(matching_folders(root, parent, segment)?);
            }

            paths = matched;
        } else {
            paths = paths.iter().map(|parent| join(parent, segment)).collect();
        }
    }

    Ok(Ok(paths))
}

fn is_unsupported_glob(segment: &str) -> bool {
    segment.contains("**") || segment.contains(['?', '[', ']', '{', '}'])
}

fn matching_folders(root: &Path, parent: &str, pattern: &str) -> anyhow::Result<Vec<String>> {
    let folder = root.join(parent);
    let mut matched = Vec::new();

    if !folder.is_dir() {
        return Ok(matched);
    }

    let entries =
        fs::read_dir(&folder).with_context(|| format!("Could not read {}", folder.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| format!("Could not read {}", folder.display()))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };

        if entry.path().is_dir() && matches_segment(pattern, name) {
            matched.push(join(parent, name));
        }
    }

    Ok(matched)
}

fn matches_segment(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let [first, middle @ .., last] = parts.as_slice() else {
        unreachable!("`expand` matches only a segment that contains `*`");
    };
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };

    for part in middle {
        let Some(start) = rest.find(part) else {
            return false;
        };

        rest = &rest[start + part.len()..];
    }

    rest.ends_with(last)
}
