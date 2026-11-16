use std::ops::Range;

use thiserror::Error;

use crate::config::Selection;
use crate::layer::Language;

pub(crate) const PATH: &str = "AGENTS.md";
pub(crate) const START: &str = "<!-- stomme:start -->";
pub(crate) const END: &str = "<!-- stomme:end -->";

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum BlockError {
    #[error("has a {marker} marker without its pair; add both markers or remove the block")]
    Unpaired { marker: &'static str },
    #[error("has more than one {marker} marker; keep one block")]
    Duplicate { marker: &'static str },
    #[error("has the end marker before the start marker; swap them")]
    Reversed,
}

pub(crate) fn render(selection: &Selection, full_check: Option<&[String]>) -> String {
    let mut lines = vec![
        START.to_string(),
        "# Shared rules".to_string(),
        String::new(),
    ];

    if let Some(argv) = full_check {
        lines.push(format!(
            "- Run the full check before finishing changes: {}.",
            command_span(argv)
        ));
    }

    lines.push(format!("- {}", format_rule(selection.language)));
    lines.push("- Follow the decisions in [docs/adr](docs/adr/README.md).".to_string());
    lines.push(
        "- Follow the shared ADRs in \
         [docs/stomme/shared/adr](docs/stomme/shared/adr/README.md)."
            .to_string(),
    );
    lines.push("- Record new decisions in [docs/adr](docs/adr/README.md).".to_string());
    lines.push("- Edit an ADR in place when its details change.".to_string());
    lines.push("- Comment only what the code cannot say. Never describe history.".to_string());
    lines.push(
        "- Document decisions, not features. See \
         [ADR 0001](docs/stomme/shared/adr/0001-documentation-scope.md)."
            .to_string(),
    );
    lines.push("- Read [the general guide](docs/stomme/shared/coding.md).".to_string());
    lines.push(format!(
        "- Read [the {} guide](docs/stomme/languages/{}/guide.md).",
        language_name(selection.language),
        selection.language
    ));

    for layer in &selection.layers {
        lines.push(format!(
            "- Read [the {layer} guide](docs/stomme/layers/{layer}/guide.md)."
        ));
    }

    lines.push(END.to_string());

    let mut block = lines.join("\n");

    block.push('\n');

    block
}

pub(crate) fn find(text: &str) -> Result<Option<Range<usize>>, BlockError> {
    let mut start = None;
    let mut end = None;
    let mut offset = 0;

    for line in text.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        let content = content.strip_suffix('\r').unwrap_or(content);

        if content == START {
            if start.is_some() {
                return Err(BlockError::Duplicate { marker: "start" });
            }

            start = Some(offset);
        } else if content == END {
            if end.is_some() {
                return Err(BlockError::Duplicate { marker: "end" });
            }

            end = Some(offset + line.len());
        }

        offset += line.len();
    }

    match (start, end) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) if start < end => Ok(Some(start..end)),
        (Some(_), Some(_)) => Err(BlockError::Reversed),
        (Some(_), None) => Err(BlockError::Unpaired { marker: "start" }),
        (None, Some(_)) => Err(BlockError::Unpaired { marker: "end" }),
    }
}

pub(crate) fn replace(text: &str, block: &str) -> Result<String, BlockError> {
    let Some(range) = find(text)? else {
        let mut updated = text.to_string();

        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }

        updated.push_str(block);

        return Ok(updated);
    };

    let mut updated = String::with_capacity(text.len() + block.len());

    updated.push_str(&text[..range.start]);
    updated.push_str(block);
    updated.push_str(&text[range.end..]);

    Ok(updated)
}

fn format_rule(language: Language) -> &'static str {
    match language {
        Language::Rust => "Format with `cargo fmt`.",
        Language::TypeScript => "Format with the `format` script in `package.json`.",
    }
}

fn language_name(language: Language) -> &'static str {
    match language {
        Language::Rust => "Rust",
        Language::TypeScript => "TypeScript",
    }
}

fn command_span(argv: &[String]) -> String {
    let command = argv
        .iter()
        .map(|argument| quote_argument(argument))
        .collect::<Vec<_>>()
        .join(" ");
    let ticks = longest_backtick_run(&command) + 1;
    let fence = "`".repeat(ticks);

    format!("{fence}{command}{fence}")
}

fn quote_argument(argument: &str) -> String {
    let needs_quotes = argument.contains(char::is_whitespace) || argument.contains('`');

    if needs_quotes {
        format!("\"{argument}\"")
    } else {
        argument.to_string()
    }
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;

    for character in text.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    longest
}

#[cfg(test)]
mod tests;
