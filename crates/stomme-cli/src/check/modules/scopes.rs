use std::collections::BTreeSet;

use ra_ap_rustc_lexer::TokenKind;

use super::use_paths::{
    Declaration, Token, TokenStream, UseLeaf, code_tokens, module_declaration, read_use_tree,
};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Scope {
    pub(super) keeps_exports: bool,
    pub(super) inline_path: Vec<String>,
    pub(super) inline_modules: BTreeSet<String>,
    pub(super) public_modules: BTreeSet<String>,
    pub(super) exports: Vec<Export>,
    pub(super) globs: Vec<Target>,
}

impl Scope {
    fn file() -> Self {
        Self::inline(Vec::new())
    }

    fn inline(inline_path: Vec<String>) -> Self {
        Self {
            keeps_exports: true,
            inline_path,
            inline_modules: BTreeSet::new(),
            public_modules: BTreeSet::new(),
            exports: Vec::new(),
            globs: Vec::new(),
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
            if let Some(name) = public_module_name(token, &tokens) {
                let current = *open_scopes.last().expect("the file scope stays open");
                scopes[current].public_modules.insert(name.to_string());
            }

            if let Some(Declaration { name, inline: true }) = module_declaration(token, &tokens) {
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

                for leaf in read_use_tree(&mut tokens) {
                    if is_glob(&leaf) {
                        scopes[current].globs.push(glob_target(leaf));
                    } else if let Some(export) = export(leaf) {
                        scopes[current].exports.push(export);
                    }
                }
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

fn after_visibility<'source>(
    token: &Token<'source>,
    tokens: &TokenStream<'source>,
) -> Option<TokenStream<'source>> {
    let is_public = token.kind == TokenKind::Ident && token.text == "pub";

    if !is_public {
        return None;
    }

    let mut lookahead = tokens.clone();

    if lookahead
        .peek()
        .is_some_and(|next| next.kind == TokenKind::OpenParen)
    {
        lookahead.position(|next| next.kind == TokenKind::CloseParen);
    }

    Some(lookahead)
}

fn is_public_use<'source>(token: &Token<'source>, tokens: &mut TokenStream<'source>) -> bool {
    let Some(mut lookahead) = after_visibility(token, tokens) else {
        return false;
    };

    let is_use = lookahead
        .next_if(|next| next.kind == TokenKind::Ident && next.text == "use")
        .is_some();

    if is_use {
        *tokens = lookahead;
    }

    is_use
}

fn public_module_name<'source>(
    token: &Token<'source>,
    tokens: &TokenStream<'source>,
) -> Option<&'source str> {
    let mut lookahead = after_visibility(token, tokens)?;
    let module_token = lookahead.next()?;
    let declaration = module_declaration(module_token, &lookahead)?;

    Some(declaration.name)
}

fn is_glob(leaf: &UseLeaf) -> bool {
    leaf.segments.last().is_some_and(|last| last == "*")
}

fn glob_target(mut leaf: UseLeaf) -> Target {
    leaf.segments.pop();

    target(leaf)
}

fn export(leaf: UseLeaf) -> Option<Export> {
    if leaf.alias.as_deref() == Some("_") {
        return None;
    }

    let last = leaf.segments.last()?;

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
