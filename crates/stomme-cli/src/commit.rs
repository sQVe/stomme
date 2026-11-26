pub(crate) fn is_40_character_lowercase_hex_commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(is_commit_byte)
}

fn is_commit_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
}

#[cfg(test)]
mod tests;
