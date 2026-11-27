use std::collections::{BTreeMap, BTreeSet};

use super::patch::{Hunk, RowKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoveredLine {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) text: String,
}

#[derive(Debug, Clone)]
pub(crate) struct HeadToHeadFile {
    pub(crate) head_path: String,
    pub(crate) hunks: Option<Vec<Hunk>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Removal {
    Every,
    Texts(BTreeSet<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InsertedLine {
    pub(crate) after: usize,
    pub(crate) text: String,
}

pub(crate) fn map_covered_lines(
    covered: &[CoveredLine],
    changes: &BTreeMap<String, HeadToHeadFile>,
    head_lines: &BTreeMap<String, Vec<Vec<u8>>>,
) -> Vec<CoveredLine> {
    let mut claimed: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut mapped = Vec::new();

    for line in covered {
        let Some(change) = changes.get(&line.path) else {
            mapped.push(line.clone());
            continue;
        };

        let Some(hunks) = &change.hunks else {
            continue;
        };

        let Some(mapped_line) = mapped_position(change, hunks, line, &claimed, head_lines) else {
            continue;
        };

        claimed.insert((change.head_path.clone(), mapped_line));
        mapped.push(CoveredLine {
            path: change.head_path.clone(),
            line: mapped_line,
            text: line.text.clone(),
        });
    }

    mapped
}

pub(crate) fn claimed_positions(covered: &[CoveredLine]) -> BTreeSet<(String, usize)> {
    covered
        .iter()
        .map(|line| (line.path.clone(), line.line))
        .collect()
}

pub(crate) fn uncovered_added_lines(
    path: &str,
    added: &[usize],
    claimed: &BTreeSet<(String, usize)>,
) -> BTreeSet<usize> {
    added
        .iter()
        .copied()
        .filter(|line| !claimed.contains(&(path.to_string(), *line)))
        .collect()
}

pub(crate) fn added_lines(hunks: &[Hunk]) -> Vec<usize> {
    hunks
        .iter()
        .flat_map(|hunk| &hunk.rows)
        .filter(|row| row.kind == RowKind::Added)
        .filter_map(|row| row.new_line)
        .collect()
}

pub(crate) fn scoped_removed_lines(hunks: &[Hunk], removal: &Removal) -> Vec<InsertedLine> {
    let mut lines = Vec::new();

    for hunk in hunks {
        let after = if hunk.new_count > 0 {
            hunk.new_start.saturating_sub(1)
        } else {
            hunk.new_start
        };

        for row in &hunk.rows {
            if row.kind != RowKind::Removed {
                continue;
            }

            if removal.contains(&row.text) {
                lines.push(InsertedLine {
                    after,
                    text: row.text.clone(),
                });
            }
        }
    }

    lines
}

impl Removal {
    pub(crate) fn from_hunks(hunks: &[Hunk]) -> Self {
        let texts = hunks
            .iter()
            .flat_map(|hunk| &hunk.rows)
            .filter(|row| row.kind == RowKind::Removed)
            .map(|row| row.text.clone())
            .collect();

        Removal::Texts(texts)
    }

    fn contains(&self, text: &str) -> bool {
        match self {
            Removal::Every => true,
            Removal::Texts(texts) => texts.contains(text),
        }
    }
}

pub(crate) fn map_old_line(hunks: &[Hunk], line: usize) -> Option<usize> {
    let mut shift: isize = 0;

    for hunk in hunks {
        if is_inside(hunk, line) {
            let row = hunk.rows.iter().find(|row| row.old_line == Some(line))?;
            return row.new_line;
        }

        if is_above(hunk, line) {
            shift += hunk.new_count as isize - hunk.old_count as isize;
        }
    }

    let mapped = line as isize + shift;

    usize::try_from(mapped).ok()
}

fn mapped_position(
    change: &HeadToHeadFile,
    hunks: &[Hunk],
    line: &CoveredLine,
    claimed: &BTreeSet<(String, usize)>,
    head_lines: &BTreeMap<String, Vec<Vec<u8>>>,
) -> Option<usize> {
    let direct = map_old_line(hunks, line.line);

    if let Some(direct) = direct
        && head_text(head_lines, &change.head_path, direct) == Some(line.text.as_bytes())
    {
        return Some(direct);
    }

    let matches: Vec<usize> = added_lines(hunks)
        .into_iter()
        .filter(|added| !claimed.contains(&(change.head_path.clone(), *added)))
        .filter(|added| {
            head_text(head_lines, &change.head_path, *added) == Some(line.text.as_bytes())
        })
        .collect();

    match matches.as_slice() {
        [single] => Some(*single),
        _ => None,
    }
}

fn head_text<'a>(
    head_lines: &'a BTreeMap<String, Vec<Vec<u8>>>,
    path: &str,
    line: usize,
) -> Option<&'a [u8]> {
    let lines = head_lines.get(path)?;
    let index = line.checked_sub(1)?;

    lines.get(index).map(Vec::as_slice)
}

fn is_inside(hunk: &Hunk, line: usize) -> bool {
    hunk.old_count > 0 && (hunk.old_start..hunk.old_start + hunk.old_count).contains(&line)
}

fn is_above(hunk: &Hunk, line: usize) -> bool {
    if hunk.old_count == 0 {
        line > hunk.old_start
    } else {
        hunk.old_start + hunk.old_count <= line
    }
}

#[cfg(test)]
mod tests;
