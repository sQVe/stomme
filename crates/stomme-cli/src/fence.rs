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
    pub(crate) view: Line<'a>,
}

// A heading and a fence both belong to the container they appear in, so their indentation is read
// from the content column of the enclosing list item, not from the start of the line.
pub(crate) fn unfenced_lines(text: &str) -> Vec<UnfencedLine<'_>> {
    let mut containers = Containers::new();
    let mut fence: Option<OpenFence> = None;
    let mut lines = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let view = containers.view(raw);

        if let Some(open) = &fence {
            let container_ended = view.depth < open.depth;
            let closes =
                view.relative <= MAX_BLOCK_INDENT && closes_fence(view.content, &open.fence);

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

        if let Some(open) = opening_fence(view.content) {
            fence = Some(OpenFence {
                fence: open,
                depth: view.depth,
            });
            continue;
        }

        containers.open_list_item(&view);
        lines.push(UnfencedLine { index, view });
    }

    lines
}

// A list item holds its content at a fixed column. Reading a line against the open items tells
// indented code and fenced code apart from content, which a count of leading spaces cannot do.
pub(crate) struct Containers {
    content_columns: Vec<usize>,
}

impl Containers {
    pub(crate) fn new() -> Self {
        Self {
            content_columns: Vec::new(),
        }
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

    pub(crate) fn open_list_item(&mut self, line: &Line) {
        if let Some(column) = list_content_column(line) {
            self.content_columns.push(column);
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
    let spaces = rest.bytes().take_while(|byte| *byte == b' ').count();
    let has_content = !rest[spaces..].is_empty();
    let is_wide = spaces > MAX_MARKER_SPACING;
    // A first line that holds only the marker and spaces, and a marker followed by five or more
    // spaces, both hold the item's content one space past the marker.
    let spacing = if has_content && !is_wide { spaces } else { 1 };

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
