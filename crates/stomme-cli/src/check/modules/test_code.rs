use std::collections::{BTreeMap, BTreeSet};

use ra_ap_rustc_lexer::TokenKind;

use super::use_paths::{Token, code_tokens, item_name, module_declaration};
use super::{is_crate_root, src_folder_for_file};
use crate::check::source_files::SourceFile;

#[derive(Default)]
pub(super) struct InlineModules {
    depth: usize,
    open: Vec<OpenModule>,
}

struct OpenModule {
    depth: usize,
    name: String,
    test: bool,
}

impl InlineModules {
    pub(super) fn open(&mut self, name: &str, test: bool) {
        self.open.push(OpenModule {
            depth: self.depth + 1,
            name: name.to_string(),
            test: test || self.in_test(),
        });
    }

    pub(super) fn advance(&mut self, token: &Token<'_>) {
        match token.kind {
            TokenKind::OpenBrace => self.depth += 1,
            TokenKind::CloseBrace => {
                self.depth = self.depth.saturating_sub(1);
                self.open.retain(|module| module.depth <= self.depth);
            }
            _ => {}
        }
    }

    pub(super) fn names(&self) -> Vec<String> {
        self.open.iter().map(|module| module.name.clone()).collect()
    }

    pub(super) fn in_test(&self) -> bool {
        self.open.last().is_some_and(|module| module.test)
    }
}

struct Declared {
    declarer: String,
    test: bool,
}

pub(super) fn test_files(
    sources: &[(&SourceFile, String)],
    src_folders: &BTreeSet<String>,
) -> BTreeSet<String> {
    let declarations = declarations_by_file(sources, src_folders);
    let mut paths: Vec<&str> = sources.iter().map(|(file, _)| file.path.as_str()).collect();
    paths.sort_by_key(|path| path.matches('/').count());
    let mut test_files = BTreeSet::new();

    for path in paths {
        let declared = declarations.get(path).map_or(&[][..], Vec::as_slice);

        if is_test_file(path, declared, &test_files) {
            test_files.insert(path.to_string());
        }
    }

    test_files
}

fn declarations_by_file(
    sources: &[(&SourceFile, String)],
    src_folders: &BTreeSet<String>,
) -> BTreeMap<String, Vec<Declared>> {
    let mut declarations: BTreeMap<String, Vec<Declared>> = BTreeMap::new();

    for (file, source) in sources {
        let Some(src_folder) = src_folder_for_file(src_folders, &file.path) else {
            continue;
        };
        let folder = declaring_folder(&file.path, &src_folder);

        for declaration in file_declarations(source) {
            let declared = format!("{folder}/{}.rs", declaration.segments.join("/"));

            declarations.entry(declared).or_default().push(Declared {
                declarer: file.path.clone(),
                test: declaration.test,
            });
        }
    }

    declarations
}

fn declaring_folder(path: &str, src_folder: &str) -> String {
    if is_crate_root(path, src_folder) {
        return src_folder.to_string();
    }

    path.strip_suffix(".rs").unwrap_or(path).to_string()
}

fn is_test_file(path: &str, declared: &[Declared], test_files: &BTreeSet<String>) -> bool {
    if declared.is_empty() {
        return is_below_test_file(path, test_files);
    }

    let production_declared = declared
        .iter()
        .any(|declaration| !declaration.test && !test_files.contains(&declaration.declarer));

    !production_declared
}

fn is_below_test_file(path: &str, test_files: &BTreeSet<String>) -> bool {
    path.match_indices('/')
        .any(|(index, _)| test_files.contains(&format!("{}.rs", &path[..index])))
}

struct FileDeclaration {
    segments: Vec<String>,
    test: bool,
}

// A `#[path = "..."]` attribute is not followed: the declared file is named after the module.
fn file_declarations(source: &str) -> Vec<FileDeclaration> {
    let tokens = code_tokens(source);
    let test_modules = test_modules(&tokens);
    let mut tokens = tokens.iter().peekable();
    let mut modules = InlineModules::default();
    let mut declarations = Vec::new();

    while let Some(token) = tokens.next() {
        if let Some(declaration) = module_declaration(token, &tokens) {
            let test = test_modules.contains(&token.index);

            if declaration.inline {
                modules.open(declaration.name, test);
            } else {
                let mut segments = modules.names();
                segments.push(declaration.name.to_string());
                declarations.push(FileDeclaration {
                    segments,
                    test: test || modules.in_test(),
                });
            }
        }

        modules.advance(token);
    }

    declarations
}

pub(super) fn test_modules(tokens: &[Token<'_>]) -> BTreeSet<usize> {
    let mut marked = BTreeSet::new();
    let mut pending = false;
    let mut position = 0;

    while position < tokens.len() {
        let token = &tokens[position];

        if let Some(end) = outer_attribute_end(tokens, position) {
            pending |= is_test_attribute(&tokens[position + 2..end]);
            position = end + 1;
            continue;
        }

        if is_word(token, "pub") {
            position = after_visibility(tokens, position);
            continue;
        }

        if pending && is_word(token, "mod") {
            marked.insert(token.index);
        }

        pending = false;
        position += 1;
    }

    marked
}

fn is_word(token: &Token<'_>, word: &str) -> bool {
    token.kind == TokenKind::Ident && token.text == word
}

fn outer_attribute_end(tokens: &[Token<'_>], position: usize) -> Option<usize> {
    let starts_attribute = tokens[position].kind == TokenKind::Pound
        && tokens
            .get(position + 1)
            .is_some_and(|token| token.kind == TokenKind::OpenBracket);

    if !starts_attribute {
        return None;
    }

    let mut depth = 0;

    for (offset, token) in tokens[position + 1..].iter().enumerate() {
        match token.kind {
            TokenKind::OpenBracket => depth += 1,
            TokenKind::CloseBracket => {
                depth -= 1;

                if depth == 0 {
                    return Some(position + 1 + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn after_visibility(tokens: &[Token<'_>], position: usize) -> usize {
    let next = position + 1;
    let restricted = tokens
        .get(next)
        .is_some_and(|token| token.kind == TokenKind::OpenParen);

    if !restricted {
        return next;
    }

    let close = tokens[next..]
        .iter()
        .position(|token| token.kind == TokenKind::CloseParen);

    close.map_or(tokens.len(), |offset| next + offset + 1)
}

fn is_test_attribute(inner: &[Token<'_>]) -> bool {
    let mut cursor = Cursor { tokens: inner };
    let is_cfg = cursor.next().and_then(item_name) == Some("cfg");
    let opens = cursor
        .next()
        .is_some_and(|token| token.kind == TokenKind::OpenParen);

    if !is_cfg || !opens {
        return false;
    }

    let predicate = cursor.predicate();
    cursor.skip_comma();
    let closes = cursor
        .next()
        .is_some_and(|token| token.kind == TokenKind::CloseParen);

    let selects_test = matches!(
        predicate,
        Some(Predicate::BareTest | Predicate::AllWithTest)
    );

    closes && cursor.tokens.is_empty() && selects_test
}

#[derive(PartialEq)]
enum Predicate {
    BareTest,
    AllWithTest,
    Other,
}

struct Cursor<'tokens, 'source> {
    tokens: &'tokens [Token<'source>],
}

impl<'source> Cursor<'_, 'source> {
    fn next(&mut self) -> Option<&Token<'source>> {
        let (first, rest) = self.tokens.split_first()?;
        self.tokens = rest;

        Some(first)
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens.first().map(|token| token.kind)
    }

    fn skip_comma(&mut self) {
        if self.peek_kind() == Some(TokenKind::Comma) {
            self.next();
        }
    }

    fn predicate(&mut self) -> Option<Predicate> {
        let name = item_name(self.next()?)?;

        match self.peek_kind() {
            Some(TokenKind::OpenParen) => {
                self.next();
                self.predicate_list(name == "all")
            }
            Some(TokenKind::Eq) => {
                self.next();
                self.next()?;

                Some(Predicate::Other)
            }
            _ if name == "test" => Some(Predicate::BareTest),
            _ => Some(Predicate::Other),
        }
    }

    fn predicate_list(&mut self, all: bool) -> Option<Predicate> {
        let mut has_test = false;

        while self.peek_kind() != Some(TokenKind::CloseParen) {
            has_test |= self.predicate()? == Predicate::BareTest;
            self.skip_comma();
        }

        self.next();

        if all && has_test {
            Some(Predicate::AllWithTest)
        } else {
            Some(Predicate::Other)
        }
    }
}

#[cfg(test)]
mod tests;
