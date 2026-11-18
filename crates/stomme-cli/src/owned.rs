use std::collections::{BTreeMap, BTreeSet};

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
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
        let found =
            percent_decode(anchor).is_some_and(|decoded| index.has_anchor(&resolved, &decoded));

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
    let mut next_suffix: BTreeMap<String, usize> = BTreeMap::new();

    for heading in headings(text) {
        let base = slug(&heading);
        let suffix = next_suffix.entry(base.clone()).or_default();
        let mut unique = base.clone();

        while slugs.contains(&unique) {
            *suffix += 1;
            unique = format!("{base}-{suffix}");
        }

        slugs.insert(unique);
    }

    slugs
}

fn headings(text: &str) -> Vec<String> {
    let mut headings = Vec::new();
    let mut current: Option<String> = None;

    for event in Parser::new(text) {
        match event {
            Event::Start(Tag::Heading { .. }) => current = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => headings.extend(current.take()),
            Event::Text(part) | Event::Code(part) => {
                if let Some(heading) = current.as_mut() {
                    heading.push_str(&part);
                }
            }
            _ => {}
        }
    }

    headings
}

fn percent_decode(fragment: &str) -> Option<String> {
    let bytes = fragment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut position = 0;

    while position < bytes.len() {
        if bytes[position] == b'%' {
            let high = hex_value(*bytes.get(position + 1)?)?;
            let low = hex_value(*bytes.get(position + 2)?)?;

            decoded.push(high * 16 + low);
            position += 3;
        } else {
            decoded.push(bytes[position]);
            position += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(digit: u8) -> Option<u8> {
    let value = char::from(digit).to_digit(16)?;

    u8::try_from(value).ok()
}

fn slug(heading: &str) -> String {
    let mut slug = String::with_capacity(heading.len());

    for character in heading.chars() {
        match character {
            ' ' | '-' => slug.push('-'),
            '_' => slug.push('_'),
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

pub(crate) fn link_targets(text: &str) -> Vec<String> {
    Parser::new(text)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                Some(dest_url.to_string())
            }
            _ => None,
        })
        .collect()
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
