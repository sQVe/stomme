pub(crate) const MAX_BLOCK_INDENT: usize = 3;

pub(crate) struct Fence {
    marker: u8,
    length: usize,
}

pub(crate) fn opening_fence(content: &str) -> Option<Fence> {
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

pub(crate) fn closes_fence(content: &str, fence: &Fence) -> bool {
    let length = content
        .bytes()
        .take_while(|byte| *byte == fence.marker)
        .count();
    let rest = &content[length..];

    length >= fence.length && rest.trim().is_empty()
}
