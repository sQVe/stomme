use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, strip_shebang, tokenize};

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

struct Parser<'a> {
    tokens: &'a [Token<'a>],
    index: usize,
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
    let mut paths = Vec::new();
    let mut inline_modules: Vec<(usize, String)> = Vec::new();
    let mut depth = 0;
    let mut index = 0;

    while index < tokens.len() {
        let token = &tokens[index];

        if is_mod_start(&tokens, index) {
            let name = item_name(&tokens[index + 1]).expect("a module name is an identifier");
            inline_modules.push((depth + 1, name.to_string()));
        }

        match token.kind {
            TokenKind::OpenBrace => depth += 1,
            TokenKind::CloseBrace => {
                depth -= 1;
                while inline_modules
                    .last()
                    .is_some_and(|(open_depth, _)| *open_depth > depth)
                {
                    inline_modules.pop();
                }
            }
            _ => {}
        }

        if is_path_start(&tokens, index) {
            let modules: Vec<String> = inline_modules
                .iter()
                .map(|(_, name)| name.clone())
                .collect();
            let mut parser = Parser {
                tokens: &tokens,
                index: index + 3,
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

            index = parser.index;
            continue;
        }

        index += 1;
    }

    paths
}

fn is_mod_start(tokens: &[Token<'_>], index: usize) -> bool {
    let Some(token) = tokens.get(index) else {
        return false;
    };
    let Some(name) = tokens.get(index + 1) else {
        return false;
    };
    let Some(brace) = tokens.get(index + 2) else {
        return false;
    };

    token.kind == TokenKind::Ident
        && token.text == "mod"
        && matches!(name.kind, TokenKind::Ident | TokenKind::RawIdent)
        && brace.kind == TokenKind::OpenBrace
}

fn is_path_start(tokens: &[Token<'_>], index: usize) -> bool {
    let Some(token) = tokens.get(index) else {
        return false;
    };
    let Some(first_colon) = tokens.get(index + 1) else {
        return false;
    };
    let Some(second_colon) = tokens.get(index + 2) else {
        return false;
    };

    let starts_path =
        token.kind == TokenKind::Ident && (token.text == "crate" || token.text == "super");

    starts_path && first_colon.kind == TokenKind::Colon && second_colon.kind == TokenKind::Colon
}

impl Parser<'_> {
    fn parse_segments(&mut self) -> Vec<Leaf> {
        let mut leaves = vec![Leaf {
            segments: Vec::new(),
            line: 0,
        }];

        while let Some(token) = self.tokens.get(self.index) {
            match token.kind {
                TokenKind::Ident | TokenKind::RawIdent => {
                    let name = item_name(token).expect("an identifier has a name");
                    push_segment(&mut leaves, name, token.line);
                    self.index += 1;
                }
                TokenKind::Star => {
                    push_segment(&mut leaves, "*", token.line);
                    self.index += 1;
                    break;
                }
                TokenKind::OpenBrace => {
                    self.index += 1;
                    let alternatives = self.parse_group();
                    leaves = combine(leaves, alternatives);
                    break;
                }
                _ => break,
            }

            if self.colon_colon() {
                self.index += 2;
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
                .get(self.index)
                .is_some_and(|token| token.kind == TokenKind::Comma)
            {
                self.index += 1;
            }

            let Some(token) = self.tokens.get(self.index) else {
                break;
            };

            if token.kind == TokenKind::CloseBrace {
                self.index += 1;
                break;
            }

            let mut item = self.parse_segments();
            leaves.append(&mut item);

            match self.tokens.get(self.index).map(|token| token.kind) {
                Some(TokenKind::Comma) => self.index += 1,
                Some(TokenKind::CloseBrace) => {
                    self.index += 1;
                    break;
                }
                _ => self.index += 1,
            }
        }

        leaves
    }

    fn colon_colon(&self) -> bool {
        let Some(first) = self.tokens.get(self.index) else {
            return false;
        };
        let Some(second) = self.tokens.get(self.index + 1) else {
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
