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
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hunk {
    pub(crate) old_start: usize,
    pub(crate) old_count: usize,
    pub(crate) new_start: usize,
    pub(crate) new_count: usize,
    pub(crate) rows: Vec<Row>,
}

pub(crate) fn parse_hunks(patch: &str) -> Option<Vec<Hunk>> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut old_line = 0;
    let mut new_line = 0;

    for text in patch.split('\n') {
        if text.starts_with("@@") {
            let hunk = parse_header(text)?;
            old_line = hunk.old_start;
            new_line = hunk.new_start;
            hunks.push(hunk);
            continue;
        }

        if hunks.is_empty() {
            continue;
        }

        if let Some(row) = row_for(text, &mut old_line, &mut new_line) {
            hunks.last_mut()?.rows.push(row);
        }
    }

    Some(hunks)
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

fn row_for(text: &str, old_line: &mut usize, new_line: &mut usize) -> Option<Row> {
    let marker = text.chars().next()?;
    let text = text.get(1..)?.to_string();

    match marker {
        '-' => {
            *old_line += 1;
            Some(Row {
                kind: RowKind::Removed,
                old_line: Some(*old_line - 1),
                new_line: None,
                text,
            })
        }
        '+' => {
            *new_line += 1;
            Some(Row {
                kind: RowKind::Added,
                old_line: None,
                new_line: Some(*new_line - 1),
                text,
            })
        }
        ' ' => {
            *old_line += 1;
            *new_line += 1;
            Some(Row {
                kind: RowKind::Context,
                old_line: Some(*old_line - 1),
                new_line: Some(*new_line - 1),
                text,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
