use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::Context as _;

use super::{Problem, Rule};

const ADR_DIRECTORY: &str = "docs/adr";
const INDEX_PATH: &str = "docs/adr/README.md";
const NUMBER_LENGTH: usize = 4;
const BULLETS: [&str; 3] = ["- ", "* ", "+ "];

struct AdrFile {
    name: String,
    number: String,
    text: String,
}

struct IndexEntry<'a> {
    label: &'a str,
    file: &'a str,
}

pub(super) fn check(root: &Path) -> anyhow::Result<Vec<Problem>> {
    let index_path = root.join(INDEX_PATH);

    if !index_path.is_file() {
        return Ok(Vec::new());
    }

    let index = fs::read_to_string(&index_path)
        .with_context(|| format!("Could not read {}", index_path.display()))?;
    let adrs = read_adrs(&root.join(ADR_DIRECTORY))?;

    Ok(find_problems(&adrs, &index))
}

fn read_adrs(directory: &Path) -> anyhow::Result<Vec<AdrFile>> {
    let entries = fs::read_dir(directory)
        .with_context(|| format!("Could not list {}", directory.display()))?;
    let mut adrs = Vec::new();

    for entry in entries {
        let entry = entry.with_context(|| format!("Could not list {}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(number) = adr_number(&name) else {
            continue;
        };
        let number = number.to_string();
        let path = entry.path();
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Could not read {}", path.display()))?;

        adrs.push(AdrFile { name, number, text });
    }

    adrs.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(adrs)
}

fn find_problems(adrs: &[AdrFile], index: &str) -> Vec<Problem> {
    let mut problems = duplicate_numbers(adrs);
    let retired = retired_numbers(index);
    let (entries, unparsed) = index_entries(index);

    for adr in adrs {
        let title = heading_title(&adr.text, &adr.number);

        if title.is_none() {
            problems.push(adr_problem(
                adr,
                Rule::AdrHeading,
                format!("the first heading must be `# ADR {}: Title`", adr.number),
            ));
        }

        if retired.contains(adr.number.as_str()) {
            problems.push(adr_problem(
                adr,
                Rule::AdrRetiredNumber,
                format!("number {} is listed as retired in {INDEX_PATH}", adr.number),
            ));
        }

        let listed: Vec<&IndexEntry> = entries
            .iter()
            .filter(|entry| entry.file == adr.name)
            .collect();

        if listed.len() != 1 {
            problems.push(index_problem(
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
                    Rule::AdrIndexLabel,
                    format!("labels {} as `{}`; use `{expected}`", adr.name, entry.label),
                ));
            }
        }
    }

    problems.extend(missing_files(adrs, &entries));

    for line in unparsed {
        problems.push(index_problem(
            Rule::AdrIndexEntry,
            format!("has an index line that is not `- [NNNN: Title](./NNNN-slug.md)`: {line}"),
        ));
    }

    problems
}

fn adr_problem(adr: &AdrFile, rule: Rule, message: String) -> Problem {
    Problem {
        path: format!("{ADR_DIRECTORY}/{}", adr.name),
        rule,
        message,
    }
}

fn index_problem(rule: Rule, message: String) -> Problem {
    Problem {
        path: INDEX_PATH.to_string(),
        rule,
        message,
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

fn missing_files(adrs: &[AdrFile], entries: &[IndexEntry]) -> Vec<Problem> {
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

// An indented `#` line counts as the first heading, so indentation cannot hide a wrong heading.
fn heading_title<'a>(text: &'a str, number: &str) -> Option<&'a str> {
    let heading = text
        .lines()
        .map(str::trim_start)
        .find(|line| line.starts_with('#'))?;
    let prefix = format!("# ADR {number}: ");
    let title = heading.strip_prefix(&prefix)?.trim();

    if title.is_empty() { None } else { Some(title) }
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

    for line in section(index, "## Index") {
        let line = line.trim();

        if let Some(entry) = index_entry(line) {
            entries.push(entry);
        } else if looks_like_a_link(line) {
            unparsed.push(line);
        }
    }

    (entries, unparsed)
}

fn index_entry(line: &str) -> Option<IndexEntry<'_>> {
    let entry = list_item(line)?.strip_prefix('[')?.strip_suffix(')')?;
    let (label, target) = entry.split_once("](")?;
    let file = target.strip_prefix("./").unwrap_or(target);
    let is_plain_file = !file.is_empty() && !file.contains(['/', '(', ')', ' ']);

    if is_plain_file {
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
