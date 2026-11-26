use crate::layer::Language;

const RUST_SKIPS: &[&str] = &["#[ignore]", "#[ignore ="];
const TYPESCRIPT_SKIPS: &[&str] = &[
    "it.skip(",
    "test.skip(",
    "describe.skip(",
    "suite.skip(",
    "xit(",
    "xtest(",
    "xdescribe(",
];
const RUST_SUPPRESSIONS: &[&str] = &["#[allow(", "#[expect(", "#![allow(", "#![expect("];
const TYPESCRIPT_SUPPRESSIONS: &[&str] = &[
    "// eslint-disable",
    "/* eslint-disable",
    "// oxlint-disable",
    "/* oxlint-disable",
    "// @ts-ignore",
    "// @ts-expect-error",
    "// @ts-nocheck",
];
const RUST_TEST_ATTRIBUTES: &[&str] = &["test", "rstest", "test_case", "quickcheck", "proptest"];
const TYPESCRIPT_CASES: &[&str] = &[
    "it(",
    "test(",
    "it.skip(",
    "test.skip(",
    "it.only(",
    "test.only(",
    "xit(",
    "xtest(",
];

pub(super) struct Declaration {
    pub(super) path: String,
    pub(super) name: String,
    pub(super) line: usize,
}

pub(super) enum Form {
    Skip,
    Suppression,
}

impl Form {
    pub(super) fn message(&self, line: usize, marker: &str) -> String {
        match self {
            Self::Skip => {
                format!("line {line}: adds the test skip `{marker}`; keep the test running")
            }
            Self::Suppression => format!(
                "line {line}: adds the suppression `{marker}`; fix the finding instead of hiding it"
            ),
        }
    }
}

pub(super) fn added_marker(line: &str, language: Language) -> Option<(&'static str, Form)> {
    let trimmed = line.trim();

    for marker in skips(language) {
        if trimmed.starts_with(marker) {
            return Some((marker, Form::Skip));
        }
    }

    for marker in suppressions(language) {
        if trimmed.starts_with(marker) {
            return Some((marker, Form::Suppression));
        }
    }

    None
}

fn skips(language: Language) -> &'static [&'static str] {
    match language {
        Language::Rust => RUST_SKIPS,
        Language::TypeScript => TYPESCRIPT_SKIPS,
    }
}

fn suppressions(language: Language) -> &'static [&'static str] {
    match language {
        Language::Rust => RUST_SUPPRESSIONS,
        Language::TypeScript => TYPESCRIPT_SUPPRESSIONS,
    }
}

pub(super) fn declarations(text: &str, language: Language, path: &str) -> Vec<Declaration> {
    match language {
        Language::Rust => rust_declarations(text, path),
        Language::TypeScript => typescript_declarations(text, path),
    }
}

fn rust_declarations(text: &str, path: &str) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    let mut pending = false;
    let mut attribute_depth = 0;

    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        if attribute_depth > 0 {
            attribute_depth = next_depth(attribute_depth, trimmed);
            continue;
        }

        if pending {
            if is_skipped(trimmed) {
                continue;
            }

            if trimmed.starts_with("#[") {
                attribute_depth = next_depth(0, trimmed);
                continue;
            }

            if let Some(name) = function_name(trimmed) {
                declarations.push(Declaration {
                    path: path.to_string(),
                    name,
                    line: index + 1,
                });
            }

            pending = false;
            continue;
        }

        if is_test_attribute(trimmed) {
            pending = true;
            attribute_depth = next_depth(0, trimmed);
        }
    }

    declarations
}

fn next_depth(mut depth: usize, line: &str) -> usize {
    for character in line.chars() {
        match character {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }

    depth
}

fn is_skipped(trimmed: &str) -> bool {
    trimmed.is_empty() || trimmed.starts_with("//")
}

fn is_test_attribute(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("#[") else {
        return false;
    };
    let path = rest.split(['(', ']', '=', ' ']).next().unwrap_or(rest);

    path.split("::")
        .any(|segment| RUST_TEST_ATTRIBUTES.contains(&segment))
}

fn function_name(trimmed: &str) -> Option<String> {
    let rest = after_fn_keyword(trimmed)?;
    let rest = skip_comments(rest)?;
    let name = identifier(rest)?;

    Some(name.to_string())
}

fn after_fn_keyword(trimmed: &str) -> Option<&str> {
    let mut search = 0;

    while let Some(found) = trimmed[search..].find("fn") {
        let start = search + found;
        let end = start + 2;
        let before = trimmed[..start].chars().next_back();
        let after = trimmed[end..].chars().next();

        if !is_identifier_character(before) && !is_identifier_character(after) {
            return Some(trimmed[end..].trim_start());
        }

        search = end;
    }

    None
}

fn is_identifier_character(character: Option<char>) -> bool {
    character.is_some_and(|character| character.is_alphanumeric() || character == '_')
}

fn skip_comments(rest: &str) -> Option<&str> {
    let mut rest = rest;

    loop {
        if rest.starts_with("//") {
            return None;
        }

        if let Some(after) = rest.strip_prefix("/*") {
            let (_, after) = after.split_once("*/")?;
            rest = after.trim_start();
            continue;
        }

        return Some(rest);
    }
}

fn identifier(rest: &str) -> Option<&str> {
    let end = rest
        .find(|character: char| character == '(' || character == '<' || character.is_whitespace())
        .unwrap_or(rest.len());
    let name = &rest[..end];

    if name.is_empty() {
        return None;
    }

    Some(name)
}

fn typescript_declarations(text: &str, path: &str) -> Vec<Declaration> {
    let mut declarations = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        if !TYPESCRIPT_CASES
            .iter()
            .any(|marker| trimmed.starts_with(marker))
        {
            continue;
        }

        let Some(name) = case_name(trimmed) else {
            continue;
        };

        declarations.push(Declaration {
            path: path.to_string(),
            name,
            line: index + 1,
        });
    }

    declarations
}

fn case_name(trimmed: &str) -> Option<String> {
    let (_, rest) = trimmed.split_once('(')?;
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;

    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut name = String::new();
    let mut characters = rest[1..].chars();

    while let Some(character) = characters.next() {
        match character {
            '\\' => name.push(characters.next()?),
            other if other == quote => return Some(name),
            other => name.push(other),
        }
    }

    None
}
