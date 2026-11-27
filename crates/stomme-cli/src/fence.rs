const MAX_BLOCK_INDENT: usize = 3;
pub(crate) const INDENTED_CODE: usize = 4;

const MAX_MARKER_SPACING: usize = 4;
const TAB_WIDTH: usize = 4;
const MAX_ORDERED_DIGITS: usize = 9;

pub(crate) struct Line<'a> {
    indent: usize,
    pub(crate) relative: usize,
    pub(crate) depth: usize,
    pub(crate) content: &'a str,
}

pub(crate) struct UnfencedLine<'a> {
    pub(crate) index: usize,
    pub(crate) item_start: usize,
    pub(crate) raw: &'a str,
    pub(crate) view: Line<'a>,
}

// A heading and a fence both belong to the container they appear in, so their indentation is read
// from the content column of the enclosing list item, not from the start of the line.
pub(crate) fn unfenced_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> impl Iterator<Item = UnfencedLine<'a>> {
    let mut containers = Containers::new();
    let mut fence: Option<OpenFence> = None;

    lines.enumerate().filter_map(move |(index, raw)| {
        let quote_limit = fence.as_ref().map_or(usize::MAX, |open| open.quotes);
        let (raw_indent, _) = split_indent(raw);
        let base = containers.base_for(raw_indent);
        let quote = split_quote(raw, quote_limit, base);
        let view = containers.view(quote.indent, quote.start, quote.content);

        if let Some(open) = &fence {
            let container_ended = view.depth < open.depth || quote.quotes < open.quotes;
            let closes = view.indent.saturating_sub(open.column) <= MAX_BLOCK_INDENT
                && closes_fence(view.content, &open.fence);

            if container_ended || closes {
                fence = None;
            }

            if !container_ended {
                return None;
            }
        }

        if view.relative >= INDENTED_CODE {
            return None;
        }

        let item_column = list_content_column(view.indent, view.content);

        if let Some(open) = opening_fence(content_after_markers(view.content)) {
            containers.open_list_item(&view);

            fence = Some(OpenFence {
                fence: open,
                depth: view.depth + usize::from(item_column.is_some()),
                quotes: quote.quotes,
                column: item_column.unwrap_or(view.indent - view.relative),
            });

            return None;
        }

        let item_start = containers.item_start().unwrap_or(index);

        containers.open_list_item(&view);

        Some(UnfencedLine {
            index,
            item_start,
            raw,
            view,
        })
    })
}

struct Quote<'a> {
    indent: usize,
    quotes: usize,
    start: usize,
    content: &'a str,
}

// A block quote marker is transparent to the list items and fences it holds. The marker counts as
// a column, so the content keeps its position inside the enclosing list item.
fn split_quote<'a>(raw: &'a str, limit: usize, mut base: usize) -> Quote<'a> {
    let mut column = 0;
    let mut quotes = 0;
    let mut start = 0;
    let mut rest = raw;

    loop {
        let content = rest.trim_start_matches([' ', '\t']);
        let whitespace = &rest[..rest.len() - content.len()];

        column = indentation_from(whitespace, column);

        let has_room = column.saturating_sub(base) <= MAX_BLOCK_INDENT;
        let is_quote = quotes < limit && has_room && content.starts_with('>');

        if !is_quote {
            return Quote {
                indent: column,
                quotes,
                start,
                content,
            };
        }

        let after = &content[1..];

        column += 1;
        quotes += 1;

        if let Some(after) = after.strip_prefix('\t') {
            column += TAB_WIDTH - column % TAB_WIDTH;
            rest = after;
        } else if let Some(after) = after.strip_prefix(' ') {
            column += 1;
            rest = after;
        } else {
            rest = after;
        }

        start = column;
        base = start;
    }
}

// The text a fence starts at, after every list marker the line holds.
fn content_after_markers(mut content: &str) -> &str {
    while let Some(marker) = list_marker(content) {
        let rest = &content[marker..];
        let spacing = list_spacing(rest);

        content = rest.get(spacing..).unwrap_or_default();
    }

    content
}

// A list item holds its content at a fixed column. Reading a line against the open items tells
// indented code and fenced code apart from content, which a count of leading spaces cannot do.
struct Containers {
    items: Vec<Item>,
    viewed: usize,
}

struct Item {
    column: usize,
    index: usize,
}

impl Containers {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            viewed: 0,
        }
    }

    // The content column of the innermost open item that holds a line at `indent`.
    fn base_for(&self, indent: usize) -> usize {
        self.items
            .iter()
            .rev()
            .find(|item| item.column <= indent)
            .map(|item| item.column)
            .unwrap_or(0)
    }

    // Reads the line against the open items. A blank line continues every open item, so it closes
    // none of them.
    fn view<'a>(&mut self, indent: usize, container_start: usize, content: &'a str) -> Line<'a> {
        self.viewed += 1;

        if content.is_empty() {
            return Line {
                indent,
                relative: 0,
                depth: self.items.len(),
                content,
            };
        }

        while self.items.last().is_some_and(|item| item.column > indent) {
            self.items.pop();
        }

        let base = self.items.last().map(|item| item.column).unwrap_or(0);
        let start = container_start.max(base);

        Line {
            indent,
            relative: indent - start,
            depth: self.items.len(),
            content,
        }
    }

    // The line that opened the innermost open item, which is the first line of the item a
    // continuation line belongs to.
    fn item_start(&self) -> Option<usize> {
        self.items.last().map(|item| item.index)
    }

    // Opens an item at the line `view` read last. The line index counts the lines read so far.
    fn open_list_item(&mut self, line: &Line) {
        let Some(index) = self.viewed.checked_sub(1) else {
            return;
        };

        if let Some(column) = list_content_column(line.indent, line.content) {
            self.items.push(Item { column, index });
        }
    }
}

struct OpenFence {
    fence: Fence,
    depth: usize,
    quotes: usize,
    column: usize,
}

fn indentation_from(whitespace: &str, mut column: usize) -> usize {
    for character in whitespace.chars() {
        if character == '\t' {
            column += TAB_WIDTH - column % TAB_WIDTH;
        } else {
            column += 1;
        }
    }

    column
}

fn indentation(whitespace: &str) -> usize {
    indentation_from(whitespace, 0)
}

pub(crate) fn split_indent(line: &str) -> (usize, &str) {
    let content = line.trim_start_matches([' ', '\t']);
    let indent = indentation(&line[..line.len() - content.len()]);

    (indent, content)
}

fn list_content_column(indent: usize, content: &str) -> Option<usize> {
    let marker = list_marker(content)?;
    let rest = &content[marker..];

    Some(indent + marker + list_spacing(rest))
}

// Whether a line starts a block of its own, so it cannot continue the paragraph above it.
pub(crate) fn opens_block(content: &str) -> bool {
    let is_quote_or_html = content.starts_with(['>', '<']);
    let is_heading_or_break = is_heading(content) || is_thematic_break(content);
    let starts_marker = opening_fence(content).is_some() || list_marker(content).is_some();

    is_quote_or_html || is_heading_or_break || starts_marker
}

pub(crate) fn is_heading(content: &str) -> bool {
    let hashes = content.bytes().take_while(|byte| *byte == b'#').count();
    let rest = &content[hashes..];

    let has_heading_level = (1..=6).contains(&hashes);
    let rest_is_spaced = rest.is_empty() || rest.starts_with([' ', '\t']);

    has_heading_level && rest_is_spaced
}

fn is_thematic_break(content: &str) -> bool {
    let Some(marker) = content.bytes().find(|byte| !byte.is_ascii_whitespace()) else {
        return false;
    };
    let is_break_marker = matches!(marker, b'-' | b'*' | b'_');
    let marks = content.bytes().filter(|byte| *byte == marker).count();
    let only_marks = content
        .bytes()
        .all(|byte| byte == marker || byte == b' ' || byte == b'\t');

    is_break_marker && marks >= 3 && only_marks
}

// The column a list item's content starts at, relative to its marker.
fn list_spacing(rest: &str) -> usize {
    let spaces = rest.bytes().take_while(|byte| *byte == b' ').count();
    let has_content = !rest[spaces..].is_empty();
    let is_wide = spaces > MAX_MARKER_SPACING;
    // A first line that holds only the marker and spaces, and a marker followed by five or more
    // spaces, both hold the item's content one space past the marker.
    if has_content && !is_wide { spaces } else { 1 }
}

// The column where the content of a list item on `line` starts, or None when it opens no item.
pub(crate) fn item_content_column(line: &str) -> Option<usize> {
    let (indent, content) = split_indent(line);

    list_content_column(indent, content)
}

pub(crate) fn list_item_content(line: &str) -> Option<&str> {
    let (_, content) = split_indent(line);
    let marker = list_marker(content)?;
    let rest = &content[marker..];

    if rest.is_empty() {
        return Some(rest);
    }

    Some(&rest[list_spacing(rest)..])
}

pub(crate) fn list_marker(content: &str) -> Option<usize> {
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
}

fn opening_fence(content: &str) -> Option<Fence> {
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

    Some(Fence { marker, length })
}

fn closes_fence(content: &str, fence: &Fence) -> bool {
    let length = content
        .bytes()
        .take_while(|byte| *byte == fence.marker)
        .count();
    let rest = &content[length..];

    length >= fence.length && rest.trim().is_empty()
}

#[cfg(test)]
mod tests;
