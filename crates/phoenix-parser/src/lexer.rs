use phoenix_ast::Span;

use crate::error::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind<'a> {
    Ident(&'a str),
    /// `\` or `λ`
    Lambda,
    In,
    Dot,
    /// `::=`
    ColonColonEqual,
    Colon,
    Arrow,
    LParen,
    RParen,
    Eof,
}

impl std::fmt::Display for TokenKind<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Ident(name) => write!(f, "identifier `{name}`"),
            TokenKind::Lambda => write!(f, "`\\`"),
            TokenKind::In => write!(f, "`in`"),
            TokenKind::Dot => write!(f, "`.`"),
            TokenKind::ColonColonEqual => write!(f, "`::=`"),
            TokenKind::Colon => write!(f, "`:`"),
            TokenKind::Arrow => write!(f, "`->`"),
            TokenKind::LParen => write!(f, "`(`"),
            TokenKind::RParen => write!(f, "`)`"),
            TokenKind::Eof => write!(f, "eof"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() && c != 'λ' || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    (c.is_alphanumeric() && c != 'λ') || c == '_' || c == '\''
}

/// Splits the source into tokens. The last one is always [`TokenKind::Eof`].
///
/// Comments start with `--` and run to the end of the line.
pub fn tokenize(src: &str) -> Result<Vec<Token<'_>>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = src.char_indices().peekable();

    while let Some(&(start, c)) = chars.peek() {
        let single = |kind| Token {
            kind,
            span: Span::new(start, start + c.len_utf8()),
        };

        match c {
            _ if c.is_whitespace() => {
                chars.next();
            }
            '-' => {
                chars.next();
                match chars.peek() {
                    Some(&(_, '-')) => {
                        // Comment: skip to the end of the line.
                        while chars.next_if(|&(_, c)| c != '\n').is_some() {}
                    }
                    Some(&(_, '>')) => {
                        chars.next();
                        tokens.push(Token {
                            kind: TokenKind::Arrow,
                            span: Span::new(start, start + 2),
                        });
                    }
                    _ => {
                        return Err(ParseError::new(
                            "Unexpected character `-` (Did you intend to write `->` ?)",
                            Span::new(start, start + 1),
                        ));
                    }
                }
            }
            '\\' | 'λ' => {
                chars.next();
                tokens.push(single(TokenKind::Lambda));
            }
            '.' => {
                chars.next();
                tokens.push(single(TokenKind::Dot));
            }
            ':' => {
                chars.next();
                if chars.next_if(|&(_, c)| c == ':').is_none() {
                    tokens.push(single(TokenKind::Colon));
                } else if chars.next_if(|&(_, c)| c == '=').is_some() {
                    tokens.push(Token {
                        kind: TokenKind::ColonColonEqual,
                        span: Span::new(start, start + 3),
                    });
                } else {
                    return Err(ParseError::new(
                        "Unexpected `::` (Did you intend to write `::=` ?)",
                        Span::new(start, start + 2),
                    ));
                }
            }
            '(' => {
                chars.next();
                tokens.push(single(TokenKind::LParen));
            }
            ')' => {
                chars.next();
                tokens.push(single(TokenKind::RParen));
            }
            _ if is_ident_start(c) => {
                let mut end = start;
                while let Some((i, c)) = chars.next_if(|&(_, c)| is_ident_continue(c)) {
                    end = i + c.len_utf8();
                }
                let text = &src[start..end];
                let kind = match text {
                    "in" => TokenKind::In,
                    _ => TokenKind::Ident(text),
                };
                tokens.push(Token {
                    kind,
                    span: Span::new(start, end),
                });
            }
            _ => {
                return Err(ParseError::new(
                    format!("Unexpected character `{c}`"),
                    Span::new(start, start + c.len_utf8()),
                ));
            }
        }
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(src.len(), src.len()),
    });
    Ok(tokens)
}
