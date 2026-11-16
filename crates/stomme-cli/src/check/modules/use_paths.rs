use std::iter::Peekable;
use std::slice::Iter;

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, strip_shebang, tokenize};

type TokenStream<'a> = Peekable<Iter<'a, Token<'a>>>;

struct Token<'a> {
    kind: TokenKind,
    text: &'a str,
    line: usize,
}

pub(super) struct UsePath {
    pub(super) segments: Vec<String>,
    pub(super) line: usize,
    pub(super) inline_modules: Vec<String>,
}

struct Leaf {
    segments: Vec<String>,
    line: usize,
}

struct Parser<'tokens, 'source> {
    tokens: &'tokens mut TokenStream<'source>,
}

fn code_tokens(source: &str) -> Vec<Token<'_>> {
    let shebang_end = strip_shebang(source).unwrap_or(0);
    let mut line = 1;
    let mut start = shebang_end;
    let mut tokens = Vec::new();

    for token in tokenize(&source[shebang_end..], FrontmatterAllowed::Yes) {
        let end = start + token.len as usize;
        let text = &source[start..end];
        let is_trivia = matches!(
            token.kind,
            TokenKind::Whitespace
                | TokenKind::LineComment { .. }
                | TokenKind::BlockComment { .. }
                | TokenKind::Frontmatter { .. }
        );

        if !is_trivia {
            tokens.push(Token {
                kind: token.kind,
                text,
                line,
            });
        }

        line += text.matches('\n').count();
        start = end;
    }

    tokens
}

pub(super) fn paths(source: &str) -> Vec<UsePath> {
    let tokens = code_tokens(source);
    let mut tokens = tokens.iter().peekable();
    let mut paths = Vec::new();
    let mut inline_modules: Vec<(usize, String)> = Vec::new();
    let mut depth = 0;

    while let Some(token) = tokens.next() {
        if let Some(name) = module_name(token, &tokens) {
            inline_modules.push((depth + 1, name.to_string()));
        }

        match token.kind {
            TokenKind::OpenBrace => depth += 1,
            TokenKind::CloseBrace => {
                depth = depth.saturating_sub(1);
                let last_kept = inline_modules
                    .iter()
                    .rposition(|(open_depth, _)| *open_depth <= depth);
                inline_modules.truncate(last_kept.map_or(0, |position| position + 1));
            }
            _ => {}
        }

        if is_path_start(token, &tokens) {
            let modules: Vec<String> = inline_modules
                .iter()
                .map(|(_, name)| name.clone())
                .collect();
            tokens
                .next()
                .expect("a checked path start has its first colon");
            tokens
                .next()
                .expect("a checked path start has its second colon");
            let mut parser = Parser {
                tokens: &mut tokens,
            };
            let leaves = parser.parse_segments();

            for leaf in leaves {
                let mut segments = vec![token.text.to_string()];
                segments.extend(leaf.segments);
                paths.push(UsePath {
                    segments,
                    line: leaf.line,
                    inline_modules: modules.clone(),
                });
            }
        }
    }

    paths
}

fn module_name<'source>(
    token: &Token<'source>,
    tokens: &TokenStream<'source>,
) -> Option<&'source str> {
    if token.kind != TokenKind::Ident || token.text != "mod" {
        return None;
    }

    let mut lookahead = tokens.clone();
    let name = lookahead.next()?;
    let brace = lookahead.next()?;
    let is_name = matches!(name.kind, TokenKind::Ident | TokenKind::RawIdent);

    if !is_name || brace.kind != TokenKind::OpenBrace {
        return None;
    }

    item_name(name)
}

fn is_path_start<'source>(token: &Token<'source>, tokens: &TokenStream<'source>) -> bool {
    let starts_path =
        token.kind == TokenKind::Ident && (token.text == "crate" || token.text == "super");

    if !starts_path {
        return false;
    }

    let mut lookahead = tokens.clone();
    let Some(first_colon) = lookahead.next() else {
        return false;
    };
    let Some(second_colon) = lookahead.next() else {
        return false;
    };

    first_colon.kind == TokenKind::Colon && second_colon.kind == TokenKind::Colon
}

impl Parser<'_, '_> {
    fn parse_segments(&mut self) -> Vec<Leaf> {
        let mut leaves = vec![Leaf {
            segments: Vec::new(),
            line: 0,
        }];

        while let Some(kind) = self.tokens.peek().map(|token| token.kind) {
            match kind {
                TokenKind::Ident | TokenKind::RawIdent => {
                    let token = self.tokens.next().expect("a peeked token is present");
                    let name = item_name(token).expect("an identifier has a name");
                    push_segment(&mut leaves, name, token.line);
                }
                TokenKind::Star => {
                    let token = self.tokens.next().expect("a peeked token is present");
                    push_segment(&mut leaves, "*", token.line);
                    break;
                }
                TokenKind::OpenBrace => {
                    self.tokens.next().expect("a peeked token is present");
                    let alternatives = self.parse_group();
                    leaves = combine(leaves, alternatives);
                    break;
                }
                _ => break,
            }

            if self.colon_colon() {
                self.tokens.next().expect("a peeked token is present");
                self.tokens.next().expect("a peeked token is present");
            } else {
                break;
            }
        }

        leaves
    }

    fn parse_group(&mut self) -> Vec<Leaf> {
        let mut leaves = Vec::new();

        loop {
            while self
                .tokens
                .peek()
                .is_some_and(|token| token.kind == TokenKind::Comma)
            {
                self.tokens.next().expect("a peeked comma is present");
            }

            let Some(kind) = self.tokens.peek().map(|token| token.kind) else {
                break;
            };

            if kind == TokenKind::CloseBrace {
                self.tokens.next().expect("a peeked close brace is present");
                break;
            }

            let mut item = self.parse_segments();
            self.skip_alias();

            for leaf in &mut item {
                if leaf.segments == ["self"] {
                    leaf.segments.clear();
                }
            }

            leaves.append(&mut item);

            match self.tokens.peek().map(|token| token.kind) {
                Some(TokenKind::CloseBrace) => {
                    self.tokens.next().expect("a peeked close brace is present");
                    break;
                }
                Some(_) => {
                    self.tokens.next().expect("a peeked token is present");
                }
                None => break,
            }
        }

        leaves
    }

    fn skip_alias(&mut self) {
        let is_alias = self.tokens.peek().is_some_and(|token| token.text == "as");

        if !is_alias {
            return;
        }

        self.tokens
            .next()
            .expect("a peeked alias keyword is present");
        self.tokens.next();
    }

    fn colon_colon(&self) -> bool {
        let mut lookahead = self.tokens.clone();
        let Some(first) = lookahead.next() else {
            return false;
        };
        let Some(second) = lookahead.next() else {
            return false;
        };

        first.kind == TokenKind::Colon && second.kind == TokenKind::Colon
    }
}

fn push_segment(leaves: &mut [Leaf], name: &str, line: usize) {
    for leaf in leaves {
        if leaf.segments.is_empty() {
            leaf.line = line;
        }

        leaf.segments.push(name.to_string());
    }
}

fn combine(prefixes: Vec<Leaf>, alternatives: Vec<Leaf>) -> Vec<Leaf> {
    let mut combined = Vec::new();

    for prefix in &prefixes {
        for alternative in &alternatives {
            let mut segments = prefix.segments.clone();
            segments.extend(alternative.segments.clone());
            let line = if prefix.segments.is_empty() {
                alternative.line
            } else {
                prefix.line
            };

            combined.push(Leaf { segments, line });
        }
    }

    combined
}

fn item_name<'a>(token: &Token<'a>) -> Option<&'a str> {
    match token.kind {
        TokenKind::Ident => Some(token.text),
        TokenKind::RawIdent => token.text.strip_prefix("r#"),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
