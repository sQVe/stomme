use std::collections::{BTreeMap, BTreeSet};
use std::iter::Peekable;
use std::slice::Iter;

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, strip_shebang, tokenize};

use super::Edition;
use super::scopes::{self, Binding, Scope};
use super::test_code::{InlineModules, test_modules};

pub(super) type TokenStream<'a> = Peekable<Iter<'a, Token<'a>>>;

pub(super) struct Token<'a> {
    pub(super) kind: TokenKind,
    pub(super) text: &'a str,
    pub(super) index: usize,
    line: usize,
}

pub(super) struct UsePath {
    pub(super) segments: Vec<String>,
    pub(super) line: usize,
    pub(super) inline_modules: Vec<String>,
    pub(super) in_test_module: bool,
    pub(super) namespace: Namespace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Namespace {
    Type,
    Value,
}

pub(super) struct Declaration<'source> {
    pub(super) name: &'source str,
    pub(super) inline: bool,
}

pub(super) struct UseLeaf {
    pub(super) absolute: bool,
    pub(super) segments: Vec<String>,
    pub(super) alias: Option<String>,
    pub(super) line: usize,
    pub(super) has_separator: bool,
}

struct Leaf {
    absolute: bool,
    segments: Vec<String>,
    alias: Option<String>,
    line: usize,
    has_separator: bool,
}

struct Parser<'tokens, 'source> {
    tokens: &'tokens mut TokenStream<'source>,
}

pub(super) fn code_tokens(source: &str) -> Vec<Token<'_>> {
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
                index: tokens.len(),
                line,
            });
        }

        line += text.matches('\n').count();
        start = end;
    }

    tokens
}

pub(super) fn read_use_tree(tokens: &mut TokenStream<'_>) -> Vec<UseLeaf> {
    let mut parser = Parser { tokens };

    parser
        .read_aliased_segments()
        .into_iter()
        .map(|leaf| UseLeaf {
            absolute: leaf.absolute,
            segments: leaf.segments,
            alias: leaf.alias,
            line: leaf.line,
            has_separator: leaf.has_separator,
        })
        .collect()
}

type BindingTable = BTreeMap<Vec<String>, BTreeMap<String, Binding>>;

pub(super) fn paths(source: &str, edition: Edition) -> Vec<UsePath> {
    let all_tokens = code_tokens(source);
    let test_modules = test_modules(&all_tokens);
    let table = binding_table(scopes::scopes(source, edition));
    let mut tokens = all_tokens.iter().peekable();
    let mut paths = Vec::new();
    let mut inline_modules = InlineModules::default();

    while let Some(token) = tokens.next() {
        if let Some(declaration) = module_declaration(token, &tokens)
            && declaration.inline
        {
            inline_modules.open(declaration.name, test_modules.contains(&token.index));
        }

        inline_modules.advance(token);

        if is_use_keyword(token) {
            read_use_declaration(&mut tokens, &mut paths, &inline_modules, &table, edition);
            continue;
        }

        if is_leading_colon_colon(token, &tokens, &all_tokens) {
            push_leading_colon_paths(&mut tokens, &mut paths, &inline_modules, edition);
            continue;
        }

        if previous_is_colon_colon(token, &all_tokens) {
            continue;
        }

        push_value_path(
            token,
            &mut tokens,
            &mut paths,
            &inline_modules,
            &table,
            edition,
        );
    }

    paths
}

fn push_leading_colon_paths(
    tokens: &mut TokenStream<'_>,
    paths: &mut Vec<UsePath>,
    inline_modules: &InlineModules,
    edition: Edition,
) {
    tokens.next().expect("a leading path has its second colon");

    if !edition.is_2015() {
        return;
    }

    let modules = inline_modules.names();
    let in_test_module = inline_modules.in_test();
    let mut parser = Parser { tokens };
    let leaves = parser.parse_segments();

    for leaf in leaves {
        let mut segments = vec!["crate".to_string()];
        segments.extend(leaf.segments);
        paths.push(UsePath {
            segments,
            line: leaf.line,
            inline_modules: modules.clone(),
            in_test_module,
            namespace: Namespace::Value,
        });
    }
}

fn push_value_path(
    token: &Token<'_>,
    tokens: &mut TokenStream<'_>,
    paths: &mut Vec<UsePath>,
    inline_modules: &InlineModules,
    table: &BindingTable,
    edition: Edition,
) {
    let inline_path = inline_modules.names();
    let Some(root) = path_root(token, tokens, &inline_path, table, edition) else {
        return;
    };
    let from_binding = !is_root_segment(token);
    let in_test_module = inline_modules.in_test();
    tokens
        .next()
        .expect("a checked path start has its first colon");
    tokens
        .next()
        .expect("a checked path start has its second colon");
    let mut parser = Parser { tokens };
    let leaves = parser.parse_segments();

    for leaf in leaves {
        let mut segments = root.clone();
        segments.extend(leaf.segments);
        let line = if from_binding { token.line } else { leaf.line };
        paths.push(UsePath {
            segments,
            line,
            inline_modules: inline_path.clone(),
            in_test_module,
            namespace: Namespace::Value,
        });
    }
}

fn binding_table(scopes: Vec<Scope>) -> BindingTable {
    let mut table = BindingTable::new();

    for scope in scopes {
        let bindings = table.entry(scope.inline_path).or_default();

        for binding in scope.bindings {
            bindings.insert(binding.name.clone(), binding);
        }
    }

    table
}

fn read_use_declaration(
    tokens: &mut TokenStream<'_>,
    paths: &mut Vec<UsePath>,
    inline_modules: &InlineModules,
    table: &BindingTable,
    edition: Edition,
) {
    let modules = inline_modules.names();
    let in_test_module = inline_modules.in_test();

    for leaf in read_use_tree(tokens) {
        let Some(segments) = use_path_segments(&leaf, edition, &modules, table) else {
            continue;
        };
        paths.push(UsePath {
            segments,
            line: leaf.line,
            inline_modules: modules.clone(),
            in_test_module,
            namespace: Namespace::Type,
        });
    }
}

fn is_use_keyword(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Ident && token.text == "use"
}

fn is_root_segment(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Ident && matches!(token.text, "crate" | "self" | "super")
}

fn is_leading_colon_colon(
    token: &Token<'_>,
    tokens: &TokenStream<'_>,
    all_tokens: &[Token<'_>],
) -> bool {
    if token.kind != TokenKind::Colon {
        return false;
    }

    let mut lookahead = tokens.clone();
    let next_is_colon = lookahead
        .peek()
        .is_some_and(|next| next.kind == TokenKind::Colon);

    if !next_is_colon {
        return false;
    }

    let previous = token
        .index
        .checked_sub(1)
        .and_then(|index| all_tokens.get(index));
    let previous_ends_a_path = previous.is_some_and(|previous| ends_a_path(previous, all_tokens));
    let previous_is_identifier = previous.is_some_and(|previous| {
        matches!(previous.kind, TokenKind::Ident | TokenKind::RawIdent)
            && !is_expression_keyword(previous)
    });

    !previous_ends_a_path && !previous_is_identifier
}

fn ends_a_path(token: &Token<'_>, all_tokens: &[Token<'_>]) -> bool {
    match token.kind {
        TokenKind::CloseBracket | TokenKind::CloseParen => true,
        TokenKind::Gt => token
            .index
            .checked_sub(1)
            .and_then(|index| all_tokens.get(index))
            .is_some_and(|before| {
                matches!(
                    before.kind,
                    TokenKind::Ident
                        | TokenKind::RawIdent
                        | TokenKind::Gt
                        | TokenKind::CloseBracket
                        | TokenKind::CloseParen
                )
            }),
        _ => false,
    }
}

fn is_expression_keyword(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Ident
        && matches!(
            token.text,
            "break" | "continue" | "else" | "if" | "in" | "match" | "return" | "while" | "yield"
        )
}

fn previous_is_colon_colon(token: &Token<'_>, all_tokens: &[Token<'_>]) -> bool {
    let Some(index) = token.index.checked_sub(2) else {
        return false;
    };
    let Some(first) = all_tokens.get(index) else {
        return false;
    };
    let Some(second) = all_tokens.get(index + 1) else {
        return false;
    };

    first.kind == TokenKind::Colon && second.kind == TokenKind::Colon
}

fn path_root(
    token: &Token<'_>,
    tokens: &TokenStream<'_>,
    inline_path: &[String],
    table: &BindingTable,
    edition: Edition,
) -> Option<Vec<String>> {
    if !followed_by_colon_colon(tokens) {
        return None;
    }

    if is_root_segment(token) {
        return Some(vec![token.text.to_string()]);
    }

    expand_root(item_name(token)?, inline_path, table, edition)
}

fn use_path_segments(
    leaf: &UseLeaf,
    edition: Edition,
    inline_path: &[String],
    table: &BindingTable,
) -> Option<Vec<String>> {
    if leaf.absolute {
        if !edition.is_2015() {
            return None;
        }

        let mut segments = vec!["crate".to_string()];
        segments.extend(leaf.segments.iter().cloned());

        return Some(segments);
    }

    if !leaf.has_separator {
        return None;
    }

    let first = leaf.segments.first()?;

    if is_root_name(first) {
        return Some(leaf.segments.clone());
    }

    let mut segments = expand_root(first, inline_path, table, edition)?;
    segments.extend(leaf.segments[1..].iter().cloned());

    Some(segments)
}

fn is_root_name(name: &str) -> bool {
    matches!(name, "crate" | "self" | "super")
}

fn expand_root(
    name: &str,
    inline_path: &[String],
    table: &BindingTable,
    edition: Edition,
) -> Option<Vec<String>> {
    let mut visited = BTreeSet::new();
    let scope = inline_path.to_vec();
    let mut current = name.to_string();
    let mut suffix: Vec<String> = Vec::new();

    loop {
        let binding = lookup_binding(table, &scope, &current)?;
        let key = (scope.clone(), current.clone());

        if !visited.insert(key) {
            return None;
        }

        let mut segments = binding.segments.clone();
        segments.extend(suffix.iter().cloned());

        if binding.absolute {
            if !edition.is_2015() {
                return None;
            }

            segments.insert(0, "crate".to_string());
            return Some(segments);
        }

        let first = segments.first()?.clone();

        if is_root_name(&first) {
            return Some(segments);
        }

        suffix = segments[1..].to_vec();
        current = first;
    }
}

fn lookup_binding<'a>(
    table: &'a BindingTable,
    inline_path: &[String],
    name: &str,
) -> Option<&'a Binding> {
    table.get(inline_path)?.get(name)
}

fn followed_by_colon_colon(tokens: &TokenStream<'_>) -> bool {
    let mut lookahead = tokens.clone();
    let Some(first_colon) = lookahead.next() else {
        return false;
    };
    let Some(second_colon) = lookahead.next() else {
        return false;
    };

    first_colon.kind == TokenKind::Colon && second_colon.kind == TokenKind::Colon
}

pub(super) fn module_declaration<'source>(
    token: &Token<'source>,
    tokens: &TokenStream<'source>,
) -> Option<Declaration<'source>> {
    if token.kind != TokenKind::Ident || token.text != "mod" {
        return None;
    }

    let mut lookahead = tokens.clone();
    let name = lookahead.next()?;
    let end = lookahead.next()?;
    let is_name = matches!(name.kind, TokenKind::Ident | TokenKind::RawIdent);

    if !is_name {
        return None;
    }

    let inline = match end.kind {
        TokenKind::OpenBrace => true,
        TokenKind::Semi => false,
        _ => return None,
    };

    Some(Declaration {
        name: item_name(name)?,
        inline,
    })
}

impl Parser<'_, '_> {
    fn parse_segments(&mut self) -> Vec<Leaf> {
        let mut leaves = vec![Leaf {
            absolute: false,
            segments: Vec::new(),
            alias: None,
            line: 0,
            has_separator: false,
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
                for leaf in &mut leaves {
                    leaf.has_separator = true;
                }

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

            let mut item = self.read_aliased_segments();

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

    fn read_aliased_segments(&mut self) -> Vec<Leaf> {
        let absolute = self.skip_leading_colon_colon();
        let mut leaves = self.parse_segments();
        let alias = self.read_alias();

        for leaf in &mut leaves {
            leaf.absolute |= absolute;

            if absolute {
                leaf.has_separator = true;
            }

            if alias.is_some() {
                leaf.alias.clone_from(&alias);
            }
        }

        leaves
    }

    fn read_alias(&mut self) -> Option<String> {
        let is_alias = self.tokens.peek().is_some_and(|token| token.text == "as");

        if !is_alias {
            return None;
        }

        self.tokens
            .next()
            .expect("a peeked alias keyword is present");
        let token = self.tokens.next()?;

        item_name(token).map(str::to_string)
    }

    fn skip_leading_colon_colon(&mut self) -> bool {
        let absolute = self.colon_colon();

        if absolute {
            self.tokens.next().expect("a peeked colon is present");
            self.tokens.next().expect("a peeked colon is present");
        }

        absolute
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

            combined.push(Leaf {
                absolute: alternative.absolute,
                segments,
                alias: alternative.alias.clone(),
                line,
                has_separator: prefix.has_separator || alternative.has_separator,
            });
        }
    }

    combined
}

pub(super) fn item_name<'a>(token: &Token<'a>) -> Option<&'a str> {
    match token.kind {
        TokenKind::Ident => Some(token.text),
        TokenKind::RawIdent => token.text.strip_prefix("r#"),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
