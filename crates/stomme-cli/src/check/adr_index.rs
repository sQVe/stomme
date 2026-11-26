use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::Context as _;

use super::Problem;
use crate::rule::Rule;

const INDEX_FILE: &str = "README.md";
const NUMBER_LENGTH: usize = 4;
const MAX_BLOCK_INDENT: usize = 3;
const INDENTED_CODE: usize = 4;
const MAX_MARKER_SPACING: usize = 4;
const TAB_WIDTH: usize = 4;
const MAX_ORDERED_DIGITS: usize = 9;
const BULLETS: [&str; 3] = ["- ", "* ", "+ "];

struct AdrFile {
    path: String,
    name: String,
    number: String,
    text: String,
}

struct IndexEntry<'a> {
    label: &'a str,
    file: &'a str,
}

pub(super) fn check(root: &Path, folder: &str) -> anyhow::Result<Vec<Problem>> {
    let index_path = root.join(folder).join(INDEX_FILE);

    if !index_path.is_file() {
        return Ok(Vec::new());
    }

    let index = fs::read_to_string(&index_path)
        .with_context(|| format!("Could not read {}", index_path.display()))?;
    let (adrs, mut problems) = read_adrs(root, folder)?;

    problems.extend(find_problems(folder, &adrs, &index));

    Ok(problems)
}

fn read_adrs(root: &Path, folder: &str) -> anyhow::Result<(Vec<AdrFile>, Vec<Problem>)> {
    let directory = root.join(folder);
    let entries = fs::read_dir(&directory)
        .with_context(|| format!("Could not list {}", directory.display()))?;
    let mut adrs = Vec::new();
    let mut problems = Vec::new();

    for entry in entries {
        let entry = entry.with_context(|| format!("Could not list {}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_path = entry.path();
        let Some(number) = adr_number(&name) else {
            if file_path.is_file() {
                problems.extend(file_name_problem(folder, &name));
            }

            continue;
        };
        let number = number.to_string();
        let text = fs::read_to_string(&file_path)
            .with_context(|| format!("Could not read {}", file_path.display()))?;
        let path = format!("{folder}/{name}");

        adrs.push(AdrFile {
            path,
            name,
            number,
            text,
        });
    }

    adrs.sort_by(|left, right| left.name.cmp(&right.name));
    problems.sort_by(|left, right| left.path.cmp(&right.path));

    Ok((adrs, problems))
}

fn find_problems(folder: &str, adrs: &[AdrFile], index: &str) -> Vec<Problem> {
    let mut problems = duplicate_numbers(adrs);
    let retired = retired_numbers(index);
    let (entries, unparsed) = index_entries(index);

    for adr in adrs {
        let title = heading_title(&adr.text, &adr.number);

        if title.is_none() {
            problems.push(adr_problem(
                adr,
                Rule::AdrHeading,
                format!(
                    "the first heading must be `# ADR {}: Title`, indented by at most 3 spaces",
                    adr.number
                ),
            ));
        }

        if retired.contains(adr.number.as_str()) {
            problems.push(adr_problem(
                adr,
                Rule::AdrRetiredNumber,
                format!(
                    "number {} is listed as retired in {folder}/{INDEX_FILE}",
                    adr.number
                ),
            ));
        }

        let listed: Vec<&IndexEntry> = entries
            .iter()
            .filter(|entry| entry.file == adr.name)
            .collect();

        if listed.len() != 1 {
            problems.push(index_problem(
                folder,
                Rule::AdrIndexCount,
                format!("lists {} {} times; list it once", adr.name, listed.len()),
            ));
        }

        let Some(title) = title else {
            continue;
        };

        let expected = format!("{}: {title}", adr.number);

        for entry in listed {
            if entry.label != expected {
                problems.push(index_problem(
                    folder,
                    Rule::AdrIndexLabel,
                    format!("labels {} as `{}`; use `{expected}`", adr.name, entry.label),
                ));
            }
        }
    }

    problems.extend(missing_files(folder, adrs, &entries));

    for line in unparsed {
        problems.push(index_problem(
            folder,
            Rule::AdrIndexEntry,
            format!(
                "has an index line that is not a `-`, `*`, or `+` list entry `[NNNN: Title](./NNNN-slug.md)`: {line}"
            ),
        ));
    }

    problems
}

fn adr_problem(adr: &AdrFile, rule: Rule, message: String) -> Problem {
    Problem {
        path: adr.path.clone(),
        rule,
        message,
        key: None,
    }
}

fn index_problem(folder: &str, rule: Rule, message: String) -> Problem {
    Problem {
        path: format!("{folder}/{INDEX_FILE}"),
        rule,
        message,
        key: None,
    }
}

fn duplicate_numbers(adrs: &[AdrFile]) -> Vec<Problem> {
    let mut files_by_number: BTreeMap<&str, Vec<&str>> = BTreeMap::new();

    for adr in adrs {
        files_by_number
            .entry(adr.number.as_str())
            .or_default()
            .push(adr.name.as_str());
    }

    adrs.iter()
        .filter_map(|adr| {
            let files = &files_by_number[adr.number.as_str()];
            let message = format!("ADR number {} is used by {}", adr.number, files.join(", "));

            (files.len() > 1).then(|| adr_problem(adr, Rule::AdrDuplicateNumber, message))
        })
        .collect()
}

fn missing_files(folder: &str, adrs: &[AdrFile], entries: &[IndexEntry]) -> Vec<Problem> {
    let names: BTreeSet<&str> = adrs.iter().map(|adr| adr.name.as_str()).collect();
    let missing: BTreeSet<&str> = entries
        .iter()
        .map(|entry| entry.file)
        .filter(|file| !names.contains(file))
        .collect();

    missing
        .into_iter()
        .map(|file| {
            index_problem(
                folder,
                Rule::AdrIndexMissingFile,
                format!("lists {file}, which does not exist"),
            )
        })
        .collect()
}

fn adr_number(name: &str) -> Option<&str> {
    let number = name.get(..NUMBER_LENGTH)?;
    let slug = name[NUMBER_LENGTH..]
        .strip_prefix('-')?
        .strip_suffix(".md")?;
    let is_number = number.bytes().all(|byte| byte.is_ascii_digit());

    if is_number && !slug.is_empty() {
        Some(number)
    } else {
        None
    }
}

fn file_name_problem(folder: &str, name: &str) -> Option<Problem> {
    if !has_empty_slug(name) {
        return None;
    }

    Some(Problem {
        path: format!("{folder}/{name}"),
        rule: Rule::AdrFileName,
        message: format!(
            "`{name}` has no slug; name an ADR file `NNNN-slug.md` with a non-empty slug"
        ),
        key: None,
    })
}

fn has_empty_slug(name: &str) -> bool {
    let Some(number) = name.get(..NUMBER_LENGTH) else {
        return false;
    };
    let Some(stem) = name[NUMBER_LENGTH..].strip_suffix(".md") else {
        return false;
    };
    let slug = stem.strip_prefix('-').unwrap_or(stem);
    let is_number = number.bytes().all(|byte| byte.is_ascii_digit());

    is_number && slug.is_empty()
}

fn heading_title<'a>(text: &'a str, number: &str) -> Option<&'a str> {
    let heading = heading_line(text)?;
    let prefix = format!("# ADR {number}: ");
    let title = heading.strip_prefix(&prefix)?.trim();

    if title.is_empty() { None } else { Some(title) }
}

// A heading and a fence both belong to the container they appear in, so their indentation is read
// from the content column of the enclosing list item, not from the start of the line.
fn heading_line(text: &str) -> Option<&str> {
    let mut containers = Containers::new();
    let mut fence: Option<Fence> = None;

    for line in text.lines() {
        let view = containers.view(line);

        if let Some(open) = &fence {
            let container_ended = view.depth < open.depth;
            let closes = view.relative <= MAX_BLOCK_INDENT && closes_fence(view.content, open);

            if container_ended || closes {
                fence = None;
            }

            if !container_ended {
                continue;
            }
        }

        if view.relative >= INDENTED_CODE {
            continue;
        }

        if let Some(open) = opening_fence(view.content, view.depth) {
            fence = Some(open);
            continue;
        }

        if view.content.starts_with('#') {
            return Some(view.content);
        }

        containers.open_list_item(&view);
    }

    None
}

// A list item holds its content at a fixed column. Reading a line against the open items tells
// indented code and fenced code apart from content, which a count of leading spaces cannot do.
struct Containers {
    content_columns: Vec<usize>,
}

struct Line<'a> {
    indent: usize,
    relative: usize,
    depth: usize,
    content: &'a str,
}

impl Containers {
    fn new() -> Self {
        Self {
            content_columns: Vec::new(),
        }
    }

    // Reads the line against the open items. A blank line continues every open item, so it closes
    // none of them.
    fn view<'a>(&mut self, raw: &'a str) -> Line<'a> {
        let leading = raw.len() - raw.trim_start_matches([' ', '\t']).len();
        let indent = indentation(&raw[..leading]);
        let content = &raw[leading..];

        if content.is_empty() {
            return Line {
                indent,
                relative: 0,
                depth: self.content_columns.len(),
                content,
            };
        }

        while self
            .content_columns
            .last()
            .is_some_and(|column| *column > indent)
        {
            self.content_columns.pop();
        }

        let base = self.content_columns.last().copied().unwrap_or(0);

        Line {
            indent,
            relative: indent - base,
            depth: self.content_columns.len(),
            content,
        }
    }

    fn open_list_item(&mut self, line: &Line) {
        if let Some(column) = list_content_column(line) {
            self.content_columns.push(column);
        }
    }
}

fn indentation(whitespace: &str) -> usize {
    whitespace.chars().fold(0, |column, character| {
        if character == '\t' {
            column + TAB_WIDTH - column % TAB_WIDTH
        } else {
            column + 1
        }
    })
}

fn list_content_column(line: &Line) -> Option<usize> {
    let marker = list_marker(line.content)?;
    let spaces = line.content[marker..]
        .bytes()
        .take_while(|byte| *byte == b' ')
        .count();
    // Five or more spaces after the marker start an indented code block in the item, so its
    // content column is one space past the marker.
    let spacing = if (1..=MAX_MARKER_SPACING).contains(&spaces) {
        spaces
    } else {
        1
    };

    Some(line.indent + marker + spacing)
}

fn list_marker(content: &str) -> Option<usize> {
    let marker = match content.as_bytes().first()? {
        b'-' | b'*' | b'+' => 1,
        b'0'..=b'9' => ordered_marker(content)?,
        _ => return None,
    };
    let rest = &content[marker..];

    if rest.is_empty() || rest.starts_with(' ') {
        Some(marker)
    } else {
        None
    }
}

fn ordered_marker(content: &str) -> Option<usize> {
    let digits = content.bytes().take_while(u8::is_ascii_digit).count();

    if digits > MAX_ORDERED_DIGITS {
        return None;
    }

    match content.as_bytes().get(digits) {
        Some(b'.' | b')') => Some(digits + 1),
        _ => None,
    }
}

struct Fence {
    marker: u8,
    length: usize,
    depth: usize,
}

fn opening_fence(content: &str, depth: usize) -> Option<Fence> {
    let marker = *content.as_bytes().first()?;
    let is_fence = marker == b'`' || marker == b'~';

    if !is_fence {
        return None;
    }

    let length = content.bytes().take_while(|byte| *byte == marker).count();
    let info = &content[length..];
    let has_backtick = marker == b'`' && info.contains('`');

    if length < 3 || has_backtick {
        return None;
    }

    Some(Fence {
        marker,
        length,
        depth,
    })
}

fn closes_fence(content: &str, fence: &Fence) -> bool {
    let length = content
        .bytes()
        .take_while(|byte| *byte == fence.marker)
        .count();
    let rest = &content[length..];

    length >= fence.length && rest.trim().is_empty()
}

fn section<'a>(text: &'a str, heading: &str) -> impl Iterator<Item = &'a str> {
    text.lines()
        .skip_while(move |line| line.trim_end() != heading)
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
}

fn index_entries(index: &str) -> (Vec<IndexEntry<'_>>, Vec<&str>) {
    let mut entries = Vec::new();
    let mut unparsed = Vec::new();
    let mut containers = Containers::new();

    for raw_line in section(index, "## Index") {
        let view = containers.view(raw_line);
        let line = view.content.trim();

        if line.is_empty() {
            continue;
        }

        if view.relative >= INDENTED_CODE {
            continue;
        }

        if let Some(entry) = index_entry(line) {
            entries.push(entry);
        } else if looks_like_a_link(line) {
            unparsed.push(line);
        }

        containers.open_list_item(&view);
    }

    (entries, unparsed)
}

fn index_entry(line: &str) -> Option<IndexEntry<'_>> {
    let entry = list_item(line)?.strip_prefix('[')?.strip_suffix(')')?;
    let (label, target) = entry.split_once("](")?;
    let file = target.strip_prefix("./").unwrap_or(target);
    let is_plain_file = !file.contains(['/', '(', ')', ' ']);
    let is_adr_file = adr_number(file).is_some();

    if is_plain_file && is_adr_file {
        Some(IndexEntry { label, file })
    } else {
        None
    }
}

fn list_item(line: &str) -> Option<&str> {
    BULLETS
        .iter()
        .find_map(|bullet| line.strip_prefix(bullet))
        .map(str::trim_start)
}

fn looks_like_a_link(line: &str) -> bool {
    let has_link_syntax = ["](", "][", "]:"]
        .iter()
        .any(|syntax| line.contains(syntax));
    let starts_with_brackets = list_item(line).unwrap_or(line).starts_with('[');

    line.contains(".md") || has_link_syntax || starts_with_brackets
}

fn retired_numbers(index: &str) -> BTreeSet<&str> {
    section(index, "## Retired")
        .filter_map(|line| line.trim().strip_prefix('|')?.split('|').next())
        .map(str::trim)
        .collect()
}

#[cfg(test)]
mod tests;
