use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::Context as _;

use super::Problem;
use crate::fence::{is_heading, unfenced_lines};
use crate::rule::Rule;

const INDEX_FILE: &str = "README.md";
const NUMBER_LENGTH: usize = 4;
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

        if !file_path.is_file() {
            continue;
        }

        let Some(number) = adr_number(&name) else {
            problems.extend(file_name_problem(folder, &name));
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
    let entries_by_file = group_by_file(&entries);

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

        let listed = entries_by_file
            .get(adr.name.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);

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

fn group_by_file<'a, 'b>(
    entries: &'b [IndexEntry<'a>],
) -> BTreeMap<&'a str, Vec<&'b IndexEntry<'a>>> {
    let mut grouped: BTreeMap<&'a str, Vec<&'b IndexEntry<'a>>> = BTreeMap::new();

    for entry in entries {
        grouped.entry(entry.file).or_default().push(entry);
    }

    grouped
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

fn heading_line(text: &str) -> Option<&str> {
    unfenced_lines(text.lines())
        .find(|line| is_heading(line.view.content))
        .map(|line| line.view.content)
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

    for line in unfenced_lines(section(index, "## Index")) {
        let text = line.view.content.trim_end();

        if text.is_empty() {
            continue;
        }

        if let Some(entry) = index_entry(text) {
            entries.push(entry);
        } else if looks_like_a_link(text) {
            unparsed.push(text);
        }
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
