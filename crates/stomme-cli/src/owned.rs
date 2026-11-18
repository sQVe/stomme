use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::config::Selection;
use crate::layer::Catalog;

pub(crate) const DOCS_FOLDER: &str = "docs/stomme";

const SHARED: &[(&str, &str)] = &[
    (
        "shared/adr/0001-documentation-scope.md",
        include_str!("../../../foundation/shared/adr/0001-documentation-scope.md"),
    ),
    (
        "shared/adr/0002-capability-modules.md",
        include_str!("../../../foundation/shared/adr/0002-capability-modules.md"),
    ),
    (
        "shared/adr/0003-owner-folders-and-file-names.md",
        include_str!("../../../foundation/shared/adr/0003-owner-folders-and-file-names.md"),
    ),
    (
        "shared/adr/0004-roles-languages-and-layers.md",
        include_str!("../../../foundation/shared/adr/0004-roles-languages-and-layers.md"),
    ),
    (
        "shared/adr/0005-results-and-parse-once.md",
        include_str!("../../../foundation/shared/adr/0005-results-and-parse-once.md"),
    ),
    (
        "shared/adr/0006-side-effect-records.md",
        include_str!("../../../foundation/shared/adr/0006-side-effect-records.md"),
    ),
    (
        "shared/adr/0007-test-placement-and-fakes.md",
        include_str!("../../../foundation/shared/adr/0007-test-placement-and-fakes.md"),
    ),
    (
        "shared/adr/0008-backwards-compatibility.md",
        include_str!("../../../foundation/shared/adr/0008-backwards-compatibility.md"),
    ),
    (
        "shared/adr/0009-advisory-review-signals.md",
        include_str!("../../../foundation/shared/adr/0009-advisory-review-signals.md"),
    ),
    (
        "shared/adr/0010-recorded-violations.md",
        include_str!("../../../foundation/shared/adr/0010-recorded-violations.md"),
    ),
    (
        "shared/adr/README.md",
        include_str!("../../../foundation/shared/adr/README.md"),
    ),
    (
        "shared/adr/TEMPLATE.md",
        include_str!("../../../foundation/shared/adr/TEMPLATE.md"),
    ),
    (
        "shared/coding.md",
        include_str!("../../../foundation/shared/coding.md"),
    ),
    (
        "shared/rules.md",
        include_str!("../../../foundation/shared/rules.md"),
    ),
];

const LANGUAGE_GUIDES: &[(&str, &str)] = &[
    (
        "languages/rust/guide.md",
        include_str!("../../../foundation/languages/rust/guide.md"),
    ),
    (
        "languages/typescript/guide.md",
        include_str!("../../../foundation/languages/typescript/guide.md"),
    ),
];

pub(crate) struct OwnedFile {
    pub(crate) path: String,
    pub(crate) content: &'static str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum LinkError {
    #[error("{path} links to {target}, which is not in the synced set")]
    Broken { path: String, target: String },
    #[error("{path} links to {target}, but {resolved} has no heading with the anchor `{anchor}`")]
    MissingAnchor {
        path: String,
        target: String,
        anchor: String,
        resolved: String,
    },
}

pub(crate) fn owned_set(selection: &Selection, catalog: &Catalog) -> Vec<OwnedFile> {
    let mut files: Vec<OwnedFile> = SHARED
        .iter()
        .map(|&(path, content)| owned(path, content))
        .collect();
    let language = format!("languages/{}/guide.md", selection.language);

    files.extend(
        LANGUAGE_GUIDES
            .iter()
            .filter(|&&(path, _)| path == language)
            .map(|&(path, content)| owned(path, content)),
    );

    for layer in &selection.layers {
        let guide = catalog
            .guide(layer)
            .expect("the selection names a registered layer");
        let path = format!("layers/{layer}/guide.md");

        files.push(owned(&path, guide));
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));

    files
}

pub(crate) fn bundle_set(catalog: &Catalog) -> Vec<OwnedFile> {
    let mut files: Vec<OwnedFile> = SHARED
        .iter()
        .chain(LANGUAGE_GUIDES)
        .map(|&(path, content)| owned(path, content))
        .collect();

    for (name, guide) in catalog.guides() {
        files.push(owned(&format!("layers/{name}/guide.md"), guide));
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));

    files
}

pub(crate) fn validate_links(files: &[OwnedFile]) -> Result<(), LinkError> {
    let index = OwnedIndex::new(files);

    for file in files {
        let base = file.path.rsplit_once('/').map_or("", |(parent, _)| parent);

        for target in link_targets(file.content) {
            check_link(file, &target, base, &index)?;
        }
    }

    Ok(())
}

struct OwnedIndex<'a> {
    contents: BTreeMap<&'a str, &'a str>,
    anchors: BTreeMap<&'a str, BTreeSet<String>>,
}

impl<'a> OwnedIndex<'a> {
    fn new(files: &'a [OwnedFile]) -> Self {
        let contents = files
            .iter()
            .map(|file| (file.path.as_str(), file.content))
            .collect();
        let anchors = files
            .iter()
            .filter(|file| file.path.ends_with(".md"))
            .map(|file| (file.path.as_str(), heading_slugs(file.content)))
            .collect();

        Self { contents, anchors }
    }

    fn contains(&self, path: &str) -> bool {
        self.contents.contains_key(path)
    }

    fn has_anchor(&self, path: &str, anchor: &str) -> bool {
        self.anchors
            .get(path)
            .is_some_and(|slugs| slugs.contains(anchor))
    }
}

fn check_link(
    file: &OwnedFile,
    target: &str,
    base: &str,
    index: &OwnedIndex<'_>,
) -> Result<(), LinkError> {
    let Some(link) = relative_link(target) else {
        return Ok(());
    };
    let Some(resolved) = resolve(base, link.path) else {
        return Err(broken_link(file, target));
    };

    if !index.contains(&resolved) {
        return Err(broken_link(file, target));
    }

    if let Some(anchor) = link.anchor
        && resolved.ends_with(".md")
    {
        let found = index.has_anchor(&resolved, anchor);

        if !found {
            return Err(LinkError::MissingAnchor {
                path: file.path.clone(),
                target: target.to_string(),
                anchor: anchor.to_string(),
                resolved,
            });
        }
    }

    Ok(())
}

fn owned(path: &str, content: &'static str) -> OwnedFile {
    OwnedFile {
        path: format!("{DOCS_FOLDER}/{path}"),
        content,
    }
}

fn broken_link(file: &OwnedFile, target: &str) -> LinkError {
    LinkError::Broken {
        path: file.path.clone(),
        target: target.to_string(),
    }
}

pub(crate) struct RelativeLink<'a> {
    pub(crate) path: &'a str,
    pub(crate) anchor: Option<&'a str>,
}

pub(crate) fn relative_link(target: &str) -> Option<RelativeLink<'_>> {
    let target = target.split_whitespace().next()?;

    if target.is_empty() || is_external(target) {
        return None;
    }

    let (path, anchor) = target
        .split_once('#')
        .map_or((target, None), |(path, anchor)| (path, Some(anchor)));

    if path.is_empty() {
        return None;
    }

    let anchor = anchor.filter(|anchor| !anchor.is_empty());

    Some(RelativeLink { path, anchor })
}

fn heading_slugs(text: &str) -> BTreeSet<String> {
    let mut slugs = BTreeSet::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    for heading in headings(text) {
        let base = slug(heading);
        let count = counts.entry(base.clone()).or_default();
        let unique = if *count == 0 {
            base
        } else {
            format!("{base}-{}", *count)
        };

        *count += 1;
        slugs.insert(unique);
    }

    slugs
}

fn headings(text: &str) -> Vec<&str> {
    let mut headings = Vec::new();
    let mut fence: Option<(u8, usize)> = None;

    for line in text.lines() {
        let bytes = line.as_bytes();

        if let Some((marker, length)) = fence {
            if let Some((found_marker, found_length, _)) = fence_start(bytes, 0)
                && found_marker == marker
                && found_length >= length
            {
                fence = None;
            }

            continue;
        }

        if let Some((marker, length, _)) = fence_start(bytes, 0) {
            fence = Some((marker, length));
            continue;
        }

        if let Some(heading) = heading_text(line) {
            headings.push(heading);
        }
    }

    headings
}

fn heading_text(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start_matches(' ').len();

    if indent > 3 {
        return None;
    }

    let rest = &line[indent..];
    let level = rest.bytes().take_while(|&byte| byte == b'#').count();

    if !(1..=6).contains(&level) {
        return None;
    }

    let after = &rest[level..];

    if !after.is_empty() && !after.starts_with(' ') {
        return None;
    }

    let text = after.trim();
    let without_closing = text.trim_end_matches('#');
    let text = if without_closing.len() < text.len() && without_closing.ends_with(' ') {
        without_closing.trim_end()
    } else {
        text
    };

    (!text.is_empty()).then_some(text)
}

fn slug(heading: &str) -> String {
    let mut slug = String::with_capacity(heading.len());

    for character in heading.chars() {
        match character {
            ' ' | '-' => slug.push('-'),
            other if other.is_alphanumeric() => slug.extend(other.to_lowercase()),
            _ => {}
        }
    }

    slug
}

fn is_external(target: &str) -> bool {
    if target.starts_with('#') || target.starts_with('/') {
        return true;
    }

    let Some((scheme, _)) = target.split_once(':') else {
        return false;
    };
    let mut characters = scheme.chars();

    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

pub(crate) fn resolve(base: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();

    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }

    Some(parts.join("/"))
}

#[derive(Clone, Copy)]
enum State {
    Normal,
    CodeSpan { ticks: usize },
    Fence { marker: u8, length: usize },
    Comment,
}

pub(crate) fn link_targets(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut targets = Vec::new();
    let mut index = 0;
    let mut at_line_start = true;
    let mut state = State::Normal;

    while index < bytes.len() {
        match state {
            State::Normal => {
                if at_line_start && let Some((marker, length, end)) = fence_start(bytes, index) {
                    state = State::Fence { marker, length };
                    index = end;
                    at_line_start = false;
                    continue;
                }

                if bytes[index] == b'`' {
                    let ticks = run_length(bytes, index, b'`');

                    state = State::CodeSpan { ticks };
                    index += ticks;
                    at_line_start = false;
                    continue;
                }

                if bytes[index..].starts_with(b"<!--") {
                    state = State::Comment;
                    index += 4;
                    at_line_start = false;
                    continue;
                }

                if bytes[index..].starts_with(b"](") {
                    let start = index + 2;

                    if let Some(end) = bytes[start..].iter().position(|&byte| byte == b')') {
                        targets.push(text[start..start + end].to_string());
                        index = start + end + 1;
                        at_line_start = false;
                        continue;
                    }
                }

                at_line_start = bytes[index] == b'\n';
                index += 1;
            }
            State::CodeSpan { ticks } => {
                if bytes[index] == b'`' {
                    let length = run_length(bytes, index, b'`');

                    if length == ticks {
                        state = State::Normal;
                    }

                    index += length;
                    at_line_start = false;
                } else {
                    at_line_start = bytes[index] == b'\n';
                    index += 1;
                }
            }
            State::Fence { marker, length } => {
                if at_line_start
                    && let Some((found_marker, found_length, end)) = fence_start(bytes, index)
                    && found_marker == marker
                    && found_length >= length
                {
                    state = State::Normal;
                    index = end;
                    at_line_start = false;
                    continue;
                }

                at_line_start = bytes[index] == b'\n';
                index += 1;
            }
            State::Comment => {
                if bytes[index..].starts_with(b"-->") {
                    state = State::Normal;
                    index += 3;
                    at_line_start = false;
                } else {
                    at_line_start = bytes[index] == b'\n';
                    index += 1;
                }
            }
        }
    }

    targets
}

fn fence_start(bytes: &[u8], index: usize) -> Option<(u8, usize, usize)> {
    let mut cursor = index;
    let mut spaces = 0;

    while cursor < bytes.len() && bytes[cursor] == b' ' && spaces < 3 {
        cursor += 1;
        spaces += 1;
    }

    let marker = *bytes.get(cursor)?;

    if marker != b'`' && marker != b'~' {
        return None;
    }

    let length = run_length(bytes, cursor, marker);

    (length >= 3).then_some((marker, length, cursor + length))
}

fn run_length(bytes: &[u8], index: usize, marker: u8) -> usize {
    bytes[index..]
        .iter()
        .take_while(|&&byte| byte == marker)
        .count()
}

#[cfg(test)]
fn embedded_paths() -> BTreeSet<String> {
    let shared = SHARED.iter().map(|&(path, _)| path.to_string());
    let languages = LANGUAGE_GUIDES.iter().map(|&(path, _)| path.to_string());
    let layers = crate::layer::catalog()
        .guides()
        .map(|(name, _)| format!("layers/{name}/guide.md"));

    shared.chain(languages).chain(layers).collect()
}

#[cfg(test)]
mod tests;
