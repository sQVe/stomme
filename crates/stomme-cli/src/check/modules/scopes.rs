use std::collections::BTreeSet;

use ra_ap_rustc_lexer::TokenKind;

use super::use_paths::{Token, TokenStream, UseLeaf, code_tokens, module_name, read_use_tree};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Scope {
    pub(super) inline_path: Vec<String>,
    pub(super) inline_modules: BTreeSet<String>,
    pub(super) exports: Vec<Export>,
}

impl Scope {
    fn file() -> Self {
        Self::inline(Vec::new())
    }

    fn inline(inline_path: Vec<String>) -> Self {
        Self {
            inline_path,
            inline_modules: BTreeSet::new(),
            exports: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Export {
    pub(super) name: String,
    pub(super) target: Target,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Target {
    External,
    Bare(Vec<String>),
    Relative {
        supers: usize,
        segments: Vec<String>,
    },
    Crate(Vec<String>),
}

#[derive(Clone, Copy)]
enum Frame {
    InlineModule,
    Other,
}

pub(super) fn scopes(source: &str) -> Vec<Scope> {
    let tokens = code_tokens(source);
    let mut tokens = tokens.iter().peekable();
    let mut scopes = vec![Scope::file()];
    let mut open_scopes = vec![0];
    let mut frames: Vec<Frame> = Vec::new();

    while let Some(token) = tokens.next() {
        let at_item_level = frames
            .iter()
            .all(|frame| matches!(frame, Frame::InlineModule));

        if at_item_level {
            if let Some(name) = module_name(token, &tokens) {
                tokens.next().expect("a checked module has its name");
                tokens.next().expect("a checked module has its brace");
                let parent = *open_scopes.last().expect("the file scope stays open");
                scopes[parent].inline_modules.insert(name.to_string());
                let mut inline_path = scopes[parent].inline_path.clone();
                inline_path.push(name.to_string());
                scopes.push(Scope::inline(inline_path));
                open_scopes.push(scopes.len() - 1);
                frames.push(Frame::InlineModule);
                continue;
            }

            if is_public_use(token, &mut tokens) {
                let current = *open_scopes.last().expect("the file scope stays open");
                let exports = read_use_tree(&mut tokens).into_iter().filter_map(export);
                scopes[current].exports.extend(exports);
                continue;
            }
        }

        match token.kind {
            TokenKind::OpenBrace | TokenKind::OpenParen | TokenKind::OpenBracket => {
                frames.push(Frame::Other);
            }
            TokenKind::CloseBrace | TokenKind::CloseParen | TokenKind::CloseBracket => {
                if let Some(Frame::InlineModule) = frames.pop() {
                    open_scopes.pop();
                }
            }
            _ => {}
        }
    }

    scopes
}

fn is_public_use<'source>(token: &Token<'source>, tokens: &mut TokenStream<'source>) -> bool {
    let is_public = token.kind == TokenKind::Ident && token.text == "pub";

    if !is_public {
        return false;
    }

    let mut lookahead = tokens.clone();

    if lookahead
        .peek()
        .is_some_and(|next| next.kind == TokenKind::OpenParen)
    {
        let restriction_end = lookahead.position(|next| next.kind == TokenKind::CloseParen);

        if restriction_end.is_none() {
            return false;
        }
    }

    let is_use = lookahead
        .next_if(|next| next.kind == TokenKind::Ident && next.text == "use")
        .is_some();

    if is_use {
        *tokens = lookahead;
    }

    is_use
}

fn export(leaf: UseLeaf) -> Option<Export> {
    if leaf.alias.as_deref() == Some("_") {
        return None;
    }

    let last = leaf.segments.last()?;

    if last == "*" {
        return None;
    }

    let name = leaf.alias.clone().unwrap_or_else(|| last.clone());

    Some(Export {
        name,
        target: target(leaf),
    })
}

fn target(leaf: UseLeaf) -> Target {
    if leaf.absolute {
        return Target::External;
    }

    let segments = leaf.segments;

    match segments.first().map(String::as_str) {
        Some("crate") => Target::Crate(segments[1..].to_vec()),
        Some("self") => Target::Relative {
            supers: 0,
            segments: segments[1..].to_vec(),
        },
        Some("super") => {
            let supers = segments
                .iter()
                .take_while(|segment| *segment == "super")
                .count();

            Target::Relative {
                supers,
                segments: segments[supers..].to_vec(),
            }
        }
        _ => Target::Bare(segments),
    }
}

#[cfg(test)]
mod tests;
