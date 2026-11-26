pub(crate) const MAX_BLOCK_INDENT: usize = 3;
pub(crate) const INDENTED_CODE: usize = 4;

const MAX_MARKER_SPACING: usize = 4;
const TAB_WIDTH: usize = 4;
const MAX_ORDERED_DIGITS: usize = 9;

pub(crate) struct Line<'a> {
    pub(crate) indent: usize,
    pub(crate) relative: usize,
    pub(crate) depth: usize,
    pub(crate) content: &'a str,
}

pub(crate) struct UnfencedLine<'a> {
    pub(crate) index: usize,
    pub(crate) item_start: usize,
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
        let view = containers.view(raw);

        if let Some(open) = &fence {
            let container_ended = view.depth < open.depth;
            let closes =
                view.relative <= MAX_BLOCK_INDENT && closes_fence(view.content, &open.fence);

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

        if let Some(open) = opening_fence(view.content) {
            fence = Some(OpenFence {
                fence: open,
                depth: view.depth,
            });
            return None;
        }

        let item_start = containers.item_start().unwrap_or(index);

        containers.open_list_item(&view, index);

        Some(UnfencedLine {
            index,
            item_start,
            view,
        })
    })
}

// A list item holds its content at a fixed column. Reading a line against the open items tells
// indented code and fenced code apart from content, which a count of leading spaces cannot do.
pub(crate) struct Containers {
    items: Vec<Item>,
}

struct Item {
    column: usize,
    line: usize,
}

impl Containers {
    pub(crate) fn new() -> Self {
        Self { items: Vec::new() }
    }

    // Reads the line against the open items. A blank line continues every open item, so it closes
    // none of them.
    pub(crate) fn view<'a>(&mut self, raw: &'a str) -> Line<'a> {
        let leading = raw.len() - raw.trim_start_matches([' ', '\t']).len();
        let indent = indentation(&raw[..leading]);
        let content = &raw[leading..];

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

        Line {
            indent,
            relative: indent - base,
            depth: self.items.len(),
            content,
        }
    }

    // The line that opened the innermost open item, which is the first line of the item a
    // continuation line belongs to.
    pub(crate) fn item_start(&self) -> Option<usize> {
        self.items.last().map(|item| item.line)
    }

    pub(crate) fn open_list_item(&mut self, line: &Line, index: usize) {
        if let Some(column) = list_content_column(line) {
            self.items.push(Item {
                column,
                line: index,
            });
        }
    }
}

struct OpenFence {
    fence: Fence,
    depth: usize,
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
    let rest = &line.content[marker..];
    let spacing = list_spacing(rest);

    Some(line.indent + marker + spacing)
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

pub(crate) fn list_item_content(line: &str) -> Option<&str> {
    let content = line.trim_start();
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
