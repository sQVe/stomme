use std::collections::BTreeSet;

use ra_ap_rustc_lexer::TokenKind;

use super::Edition;
use super::use_paths::{
    Declaration, Token, TokenStream, UseLeaf, code_tokens, item_name, module_declaration,
    read_use_tree,
};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Scope {
    pub(super) keeps_exports: bool,
    pub(super) inline_path: Vec<String>,
    pub(super) inline_modules: BTreeSet<String>,
    pub(super) public_modules: BTreeSet<String>,
    pub(super) exports: Vec<Export>,
    pub(super) globs: Vec<Glob>,
    pub(super) value_items: BTreeSet<String>,
    pub(super) bindings: Vec<Binding>,
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
            value_items: BTreeSet::new(),
            bindings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Binding {
    pub(super) name: String,
    pub(super) absolute: bool,
    pub(super) segments: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Export {
    pub(super) name: String,
    pub(super) target: Target,
    pub(super) visibility: ExportVisibility,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Glob {
    pub(super) target: Target,
    pub(super) visibility: ExportVisibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExportVisibility {
    Public,
    Crate,
    Restricted,
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

pub(super) fn scopes(source: &str, edition: Edition, module_depth: usize) -> Vec<Scope> {
    let all_tokens = code_tokens(source);
    let mut tokens = all_tokens.iter().peekable();
    let mut scopes = vec![Scope::file()];
    let mut open_scopes = vec![0];
    let mut frames: Vec<Frame> = Vec::new();

    while let Some(token) = tokens.next() {
        let at_item_level = frames
            .iter()
            .all(|frame| matches!(frame, Frame::InlineModule));

        if at_item_level {
            let current = *open_scopes.last().expect("the file scope stays open");
            let parent_is_crate_root = module_depth + scopes[current].inline_path.len() == 1;
            let previous = token
                .index
                .checked_sub(1)
                .and_then(|index| all_tokens.get(index));

            if let Some(name) = public_module_name(token, &tokens, parent_is_crate_root) {
                scopes[current].public_modules.insert(name.to_string());
            }

            if let Some(name) = value_item_name(token, &tokens, previous) {
                scopes[current].value_items.insert(name.to_string());
            }

            if let Some(Declaration { name, inline: true }) = module_declaration(token, &tokens) {
                tokens.next().expect("a checked module has its name");
                tokens.next().expect("a checked module has its brace");
                scopes[current].inline_modules.insert(name.to_string());
                let mut inline_path = scopes[current].inline_path.clone();
                inline_path.push(name.to_string());
                scopes.push(Scope::inline(inline_path));
                open_scopes.push(scopes.len() - 1);
                frames.push(Frame::InlineModule);
                continue;
            }

            if let Some(kind) = use_kind(token, &mut tokens, parent_is_crate_root) {
                for leaf in read_use_tree(&mut tokens) {
                    if let Some(binding) = binding(&leaf) {
                        scopes[current].bindings.push(binding);
                    }

                    let Some(visibility) = kind.visibility() else {
                        continue;
                    };

                    if is_glob(&leaf) {
                        scopes[current].globs.push(Glob {
                            target: glob_target(leaf, edition),
                            visibility,
                        });
                    } else if let Some(export) = export(leaf, visibility, edition) {
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
    parent_is_crate_root: bool,
) -> Option<(TokenStream<'source>, ExportVisibility)> {
    let is_public = token.kind == TokenKind::Ident && token.text == "pub";

    if !is_public {
        return None;
    }

    let mut lookahead = tokens.clone();
    let mut visibility = ExportVisibility::Public;

    if lookahead
        .peek()
        .is_some_and(|next| next.kind == TokenKind::OpenParen)
    {
        lookahead.next().expect("a peeked open paren is present");
        let mut first = lookahead.next();

        if first.is_some_and(|next| next.kind == TokenKind::Ident && next.text == "in") {
            first = lookahead.next();
        }

        let keyword = first.and_then(|next| (next.kind == TokenKind::Ident).then_some(next.text));
        let names_path = lookahead
            .peek()
            .is_some_and(|next| next.kind == TokenKind::Colon);

        lookahead.position(|next| next.kind == TokenKind::CloseParen);
        visibility = match keyword {
            Some("crate") if !names_path => ExportVisibility::Crate,
            Some("super") if !names_path && parent_is_crate_root => ExportVisibility::Crate,
            _ => ExportVisibility::Restricted,
        };
    }

    Some((lookahead, visibility))
}

#[derive(Clone, Copy)]
enum UseKind {
    Plain,
    Public(ExportVisibility),
}

impl UseKind {
    fn visibility(self) -> Option<ExportVisibility> {
        match self {
            Self::Plain => None,
            Self::Public(visibility) => Some(visibility),
        }
    }
}

fn use_kind<'source>(
    token: &Token<'source>,
    tokens: &mut TokenStream<'source>,
    parent_is_crate_root: bool,
) -> Option<UseKind> {
    if token.kind == TokenKind::Ident && token.text == "use" {
        return Some(UseKind::Plain);
    }

    let (mut lookahead, visibility) = after_visibility(token, tokens, parent_is_crate_root)?;

    let is_use = lookahead
        .next_if(|next| next.kind == TokenKind::Ident && next.text == "use")
        .is_some();

    if is_use {
        *tokens = lookahead;
        Some(UseKind::Public(visibility))
    } else {
        None
    }
}

fn public_module_name<'source>(
    token: &Token<'source>,
    tokens: &TokenStream<'source>,
    parent_is_crate_root: bool,
) -> Option<&'source str> {
    let (mut lookahead, visibility) = after_visibility(token, tokens, parent_is_crate_root)?;

    if visibility == ExportVisibility::Restricted {
        return None;
    }

    let module_token = lookahead.next()?;
    let declaration = module_declaration(module_token, &lookahead)?;

    Some(declaration.name)
}

fn value_item_name<'source>(
    token: &Token<'source>,
    tokens: &TokenStream<'source>,
    previous: Option<&Token<'source>>,
) -> Option<&'source str> {
    let keyword = item_name(token)?;
    let mut lookahead = tokens.clone();

    match keyword {
        "fn" => item_name(lookahead.next()?),
        "static" => {
            let mut next = lookahead.next()?;

            if next.text == "mut" {
                next = lookahead.next()?;
            }

            item_name(next)
        }
        "const" => {
            let follows_generic_parameter =
                previous.is_some_and(|previous| matches!(previous.text, "<" | ","));

            if follows_generic_parameter {
                return None;
            }

            let next = lookahead.next()?;
            let is_modifier = matches!(next.text, "fn" | "unsafe" | "extern" | "async");

            if is_modifier {
                return None;
            }

            item_name(next)
        }
        _ => None,
    }
}

fn is_glob(leaf: &UseLeaf) -> bool {
    leaf.segments.last().is_some_and(|last| last == "*")
}

fn binding(leaf: &UseLeaf) -> Option<Binding> {
    if is_glob(leaf) {
        return None;
    }

    if leaf.segments.is_empty() {
        return None;
    }

    let name = leaf
        .alias
        .clone()
        .or_else(|| leaf.segments.last().cloned())?;

    if name == "_" {
        return None;
    }

    Some(Binding {
        name,
        absolute: leaf.absolute,
        segments: leaf.segments.clone(),
    })
}

fn glob_target(mut leaf: UseLeaf, edition: Edition) -> Target {
    leaf.segments.pop();

    target(leaf, edition)
}

fn export(leaf: UseLeaf, visibility: ExportVisibility, edition: Edition) -> Option<Export> {
    if leaf.alias.as_deref() == Some("_") {
        return None;
    }

    let last = leaf.segments.last()?;

    let name = leaf.alias.clone().unwrap_or_else(|| last.clone());

    Some(Export {
        name,
        target: target(leaf, edition),
        visibility,
    })
}

fn target(leaf: UseLeaf, edition: Edition) -> Target {
    if leaf.absolute {
        if edition.is_2015() {
            return Target::Crate(leaf.segments);
        }

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
