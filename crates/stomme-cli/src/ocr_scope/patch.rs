#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowKind {
    Added,
    Removed,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) kind: RowKind,
    pub(crate) old_line: Option<usize>,
    pub(crate) new_line: Option<usize>,
    pub(crate) text: Vec<u8>,
    pub(crate) terminated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hunk {
    pub(crate) old_start: usize,
    pub(crate) old_count: usize,
    pub(crate) new_start: usize,
    pub(crate) new_count: usize,
    pub(crate) rows: Vec<Row>,
}

pub(crate) fn parse_hunks(patch: &[u8]) -> Option<Vec<Hunk>> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut old_line = 0;
    let mut new_line = 0;
    let mut old_left = 0;
    let mut new_left = 0;

    for text in patch.split(|byte| *byte == b'\n') {
        if text.starts_with(b"@@") {
            let hunk = parse_header(std::str::from_utf8(text).ok()?)?;
            old_line = hunk.old_start;
            new_line = hunk.new_start;
            old_left = hunk.old_count;
            new_left = hunk.new_count;
            hunks.push(hunk);
            continue;
        }

        if text.starts_with(b"\\") {
            if let Some(row) = hunks.last_mut().and_then(|hunk| hunk.rows.last_mut()) {
                row.terminated = false;
            }

            continue;
        }

        if old_left == 0 && new_left == 0 {
            continue;
        }

        let Some(marker) = text.first().copied() else {
            continue;
        };
        let Some(kind) = counted_kind(marker, old_left, new_left) else {
            continue;
        };

        if kind != RowKind::Added {
            old_left -= 1;
        }

        if kind != RowKind::Removed {
            new_left -= 1;
        }

        let row = row_for(kind, &text[1..], &mut old_line, &mut new_line);

        hunks.last_mut()?.rows.push(row);
    }

    Some(hunks)
}

fn counted_kind(marker: u8, old_left: usize, new_left: usize) -> Option<RowKind> {
    match marker {
        b'-' if old_left > 0 => Some(RowKind::Removed),
        b'+' if new_left > 0 => Some(RowKind::Added),
        b' ' if old_left > 0 && new_left > 0 => Some(RowKind::Context),
        _ => None,
    }
}

fn parse_header(text: &str) -> Option<Hunk> {
    let rest = text.strip_prefix("@@ -")?;
    let (old_start, rest) = parse_number(rest)?;
    let (old_count, rest) = parse_count(rest)?;
    let rest = rest.strip_prefix(" +")?;
    let (new_start, rest) = parse_number(rest)?;
    let (new_count, rest) = parse_count(rest)?;
    rest.strip_prefix(" @@")?;

    Some(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        rows: Vec::new(),
    })
}

fn parse_number(text: &str) -> Option<(usize, &str)> {
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    let digits = &text[..end];

    if digits.is_empty() {
        return None;
    }

    Some((digits.parse().ok()?, &text[end..]))
}

fn parse_count(text: &str) -> Option<(usize, &str)> {
    let Some(rest) = text.strip_prefix(',') else {
        return Some((1, text));
    };

    parse_number(rest)
}

fn row_for(kind: RowKind, text: &[u8], old_line: &mut usize, new_line: &mut usize) -> Row {
    let mut row = Row {
        kind,
        old_line: None,
        new_line: None,
        text: text.to_vec(),
        terminated: true,
    };

    if kind != RowKind::Added {
        *old_line += 1;
        row.old_line = Some(*old_line - 1);
    }

    if kind != RowKind::Removed {
        *new_line += 1;
        row.new_line = Some(*new_line - 1);
    }

    row
}

#[cfg(test)]
mod tests;
