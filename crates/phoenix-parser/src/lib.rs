//! Parser for Phoenix's lambda calculus.
//!
//! Grammar:
//!
//! ```text
//! expr   ::= lambda | let | app
//! lambda ::= ('\' | 'λ') param+ '.' expr
//! param  ::= IDENT | '(' IDENT+ ':' type ')'
//! let    ::= IDENT (':' type)? '::=' expr 'in' expr
//! app    ::= atom atom* lambda?
//! atom   ::= IDENT | '(' expr (':' type)? ')'
//!
//! type   ::= tatom ('->' type)?          -- right-associative
//! tatom  ::= IDENT | '(' type ')'
//! ```
//!
//! A lambda (or a `let`) extends as far right as possible. A lambda may appear
//! without parentheses as the last argument: `f \x. x`.
//!
//! `let` has no keyword: an expression starting with `x ::=`
//! or `x : T ::=` is a
//! binding. Inside parentheses, `(x : T)` without a following `::=`
//! stays a type annotation.

mod error;
mod lexer;

use phoenix_ast::{Expr, Ident, Param, PhoenixNode, Spanned, Type, TypeNode};

pub use error::ParseError;
pub use lexer::{Token, TokenKind, tokenize};

pub type Result<T> = std::result::Result<T, ParseError>;

/// Parses a whole program (a single expression).
pub fn parse_expr(src: &str) -> Result<Expr<'_>> {
    let mut parser = Parser::new(tokenize(src)?);
    let expr = parser.expr()?;
    parser.expect_eof()?;
    Ok(expr)
}

/// Parses a single type.
pub fn parse_type(src: &str) -> Result<Type<'_>> {
    let mut parser = Parser::new(tokenize(src)?);
    let ty = parser.ty()?;
    parser.expect_eof()?;
    Ok(ty)
}

struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Token<'a> {
        self.tokens[self.pos]
    }

    fn bump(&mut self) -> Token<'a> {
        let tok = self.peek();
        if tok.kind != TokenKind::Eof {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, kind: TokenKind<'a>) -> Option<Token<'a>> {
        (self.peek().kind == kind).then(|| self.bump())
    }

    fn expect(&mut self, kind: TokenKind<'a>) -> Result<Token<'a>> {
        self.eat(kind)
            .ok_or_else(|| self.unexpected(&kind.to_string()))
    }

    fn expect_eof(&mut self) -> Result<()> {
        self.expect(TokenKind::Eof).map(|_| ())
    }

    fn unexpected(&self, expected: &str) -> ParseError {
        let tok = self.peek();
        ParseError::new(format!("Expected {expected}, found {}", tok.kind), tok.span)
    }

    fn ident(&mut self) -> Result<Ident<'a>> {
        match self.peek().kind {
            TokenKind::Ident(name) => Ok(Spanned::new(name, self.bump().span)),
            _ => Err(self.unexpected("identifier")),
        }
    }

    fn expr(&mut self) -> Result<Expr<'a>> {
        match self.peek().kind {
            TokenKind::Lambda => self.lambda(),
            _ if self.at_let() => self.let_(),
            _ => self.app(),
        }
    }

    fn lambda(&mut self) -> Result<Expr<'a>> {
        let start = self.expect(TokenKind::Lambda)?.span;
        let mut params = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Ident(_) => {
                    params.push(Param {
                        name: self.ident()?,
                        annotation: None,
                    });
                }
                TokenKind::LParen => self.annotated_params(&mut params)?,
                _ => break,
            }
        }
        if params.is_empty() {
            return Err(self.unexpected("parameter"));
        }
        self.expect(TokenKind::Dot)?;
        let body = self.expr()?;
        let span = start.merge(body.span);
        Ok(Spanned::new(
            PhoenixNode::Lambda {
                params,
                body: Box::new(body),
            },
            span,
        ))
    }

    /// `(x y : T)` — each name gets its own copy of the annotation.
    fn annotated_params(&mut self, params: &mut Vec<Param<'a>>) -> Result<()> {
        self.expect(TokenKind::LParen)?;
        let mut names = vec![self.ident()?];
        while let TokenKind::Ident(_) = self.peek().kind {
            names.push(self.ident()?);
        }
        self.expect(TokenKind::Colon)?;
        let ty = self.ty()?;
        self.expect(TokenKind::RParen)?;
        params.extend(names.into_iter().map(|name| Param {
            name,
            annotation: Some(ty.clone()),
        }));
        Ok(())
    }

    /// Looks ahead for `IDENT ::=` or `IDENT : type ::=`
    /// without consuming anything.
    fn at_let(&mut self) -> bool {
        let TokenKind::Ident(_) = self.peek().kind else {
            return false;
        };
        match self.tokens[self.pos + 1].kind {
            TokenKind::ColonColonEqual => true,
            TokenKind::Colon => {
                let saved = self.pos;
                self.pos += 2;
                let is_let = self.ty().is_ok() && self.peek().kind == TokenKind::ColonColonEqual;
                self.pos = saved;
                is_let
            }
            _ => false,
        }
    }

    fn let_(&mut self) -> Result<Expr<'a>> {
        let name = self.ident()?;
        let annotation = match self.eat(TokenKind::Colon) {
            Some(_) => Some(self.ty()?),
            None => None,
        };
        self.expect(TokenKind::ColonColonEqual)?;
        let value = self.expr()?;
        self.expect(TokenKind::In)?;
        let body = self.expr()?;
        let span = name.span.merge(body.span);
        Ok(Spanned::new(
            PhoenixNode::Let {
                name,
                annotation,
                value: Box::new(value),
                body: Box::new(body),
            },
            span,
        ))
    }

    fn app(&mut self) -> Result<Expr<'a>> {
        let callee = self.atom()?;
        let mut args = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::Ident(_) | TokenKind::LParen => args.push(self.atom()?),
                TokenKind::Lambda => {
                    // Last argument: extends to the end.
                    args.push(self.expr()?);
                    break;
                }
                _ => break,
            }
        }
        if args.is_empty() {
            return Ok(callee);
        }
        let span = callee.span.merge(args.last().unwrap().span);
        Ok(Spanned::new(
            PhoenixNode::Application {
                callee: Box::new(callee),
                args,
            },
            span,
        ))
    }

    fn atom(&mut self) -> Result<Expr<'a>> {
        match self.peek().kind {
            TokenKind::Ident(name) => {
                Ok(Spanned::new(PhoenixNode::Variable(name), self.bump().span))
            }
            TokenKind::LParen => {
                let start = self.bump().span;
                let expr = self.expr()?;
                let ty = match self.eat(TokenKind::Colon) {
                    Some(_) => Some(self.ty()?),
                    None => None,
                };
                let span = start.merge(self.expect(TokenKind::RParen)?.span);
                Ok(match ty {
                    Some(ty) => Spanned::new(
                        PhoenixNode::Annotation {
                            expr: Box::new(expr),
                            ty,
                        },
                        span,
                    ),
                    // Plain parentheses do not appear in the AST,
                    // but the span includes them.
                    None => Spanned::new(expr.node, span),
                })
            }
            _ => Err(self.unexpected("expression")),
        }
    }

    // ----------------------------------------------------------------- type

    fn ty(&mut self) -> Result<Type<'a>> {
        let from = self.type_atom()?;
        if self.eat(TokenKind::Arrow).is_some() {
            let to = self.ty()?;
            let span = from.span.merge(to.span);
            return Ok(Spanned::new(
                TypeNode::Arrow(Box::new(from), Box::new(to)),
                span,
            ));
        }
        Ok(from)
    }

    fn type_atom(&mut self) -> Result<Type<'a>> {
        match self.peek().kind {
            TokenKind::Ident(name) => Ok(Spanned::new(TypeNode::Name(name), self.bump().span)),
            TokenKind::LParen => {
                let start = self.bump().span;
                let ty = self.ty()?;
                let end = self.expect(TokenKind::RParen)?.span;
                Ok(Spanned::new(ty.node, start.merge(end)))
            }
            _ => Err(self.unexpected("type")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phoenix_ast::Span;

    fn roundtrip(src: &str) -> String {
        parse_expr(src)
            .unwrap_or_else(|e| panic!("{}", e.render(src)))
            .to_string()
    }

    #[test]
    fn variable() {
        assert_eq!(parse_expr("x").unwrap().node, PhoenixNode::Variable("x"));
    }

    #[test]
    fn identity() {
        let e = parse_expr("\\x. x").unwrap();
        let PhoenixNode::Lambda { params, body } = e.node else {
            panic!()
        };
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name.node, "x");
        assert!(params[0].annotation.is_none());
        assert_eq!(body.node, PhoenixNode::Variable("x"));
        assert_eq!(e.span, Span::new(0, 5));
    }

    #[test]
    fn unicode_lambda() {
        assert_eq!(roundtrip("λx.x"), "\\x. x");
    }

    #[test]
    fn application_is_flat_and_left_assoc() {
        let e = parse_expr("f a b c").unwrap();
        let PhoenixNode::Application { callee, args } = e.node else {
            panic!()
        };
        assert_eq!(callee.node, PhoenixNode::Variable("f"));
        assert_eq!(args.len(), 3);
    }

    #[test]
    fn explicit_nested_application_is_preserved() {
        assert_eq!(roundtrip("(f a) b"), "(f a) b");
        assert_eq!(roundtrip("f (a b)"), "f (a b)");
    }

    #[test]
    fn lambda_extends_right() {
        assert_eq!(roundtrip("\\x. f x \\y. y x"), "\\x. f x \\y. y x");
        assert_eq!(roundtrip("(\\x. x) y"), "(\\x. x) y");
        assert_eq!(roundtrip("f (\\x. x) y"), "f (\\x. x) y");
    }

    #[test]
    fn annotated_params() {
        assert_eq!(
            roundtrip("\\(x y : a) z (f : a -> b). f x"),
            "\\(x : a) (y : a) z (f : a -> b). f x"
        );
    }

    #[test]
    fn types_are_right_assoc() {
        let t = parse_type("a -> b -> c").unwrap();
        let TypeNode::Arrow(from, to) = t.node else {
            panic!()
        };
        assert_eq!(from.node, TypeNode::Name("a"));
        assert!(matches!(to.node, TypeNode::Arrow(..)));
        assert_eq!(
            parse_type("(a -> b) -> c").unwrap().to_string(),
            "(a -> b) -> c"
        );
        assert_eq!(
            parse_type("a -> (b -> c)").unwrap().to_string(),
            "a -> b -> c"
        );
    }

    #[test]
    fn annotation() {
        let e = parse_expr("(\\x. x : a -> a)").unwrap();
        assert!(matches!(e.node, PhoenixNode::Annotation { .. }));
        assert_eq!(e.to_string(), "(\\x. x : a -> a)");
    }

    #[test]
    fn let_binding() {
        assert_eq!(
            roundtrip("id : a -> a ::= \\x. x in id id"),
            "id : a -> a ::= \\x. x in id id"
        );
        assert_eq!(
            roundtrip("x ::= y in z ::= x in z"),
            "x ::= y in z ::= x in z"
        );
        assert_eq!(
            roundtrip("const ::= \\x y. y in \\x. const x x"),
            "const ::= \\x y. y in \\x. const x x"
        );
        let e = parse_expr("f : (a -> b) -> c ::= g in f").unwrap();
        let PhoenixNode::Let { annotation, .. } = e.node else {
            panic!()
        };
        assert_eq!(annotation.unwrap().to_string(), "(a -> b) -> c");
    }

    #[test]
    fn annotation_vs_let_in_parens() {
        let e = parse_expr("(x : a)").unwrap();
        assert!(matches!(e.node, PhoenixNode::Annotation { .. }));
        let e = parse_expr("(x : a ::= y in x)").unwrap();
        assert!(matches!(e.node, PhoenixNode::Let { .. }));
        assert_eq!(roundtrip("f (x ::= a in x)"), "f (x ::= a in x)");
        assert!(parse_expr("f x ::= a in x").is_err());
    }

    #[test]
    fn comments_and_whitespace() {
        assert_eq!(roundtrip("-- identité\n\\x.  -- corps\n  x"), "\\x. x");
    }

    #[test]
    fn primes_in_identifiers() {
        assert_eq!(roundtrip("\\x'. x'"), "\\x'. x'");
    }

    #[test]
    fn keywords_are_not_identifiers() {
        assert!(parse_expr("\\in. in").is_err());
        assert!(parse_expr("in ::= x in x").is_err());
        assert_eq!(roundtrip("let"), "let");
    }

    #[test]
    fn errors() {
        let err = parse_expr("\\. x").unwrap_err();
        assert_eq!(err.span, Span::new(1, 2));
        assert!(parse_expr("(x").is_err());
        assert!(parse_expr("x )").is_err());
        assert!(parse_expr("x # y").is_err());
        assert!(parse_expr("\\(x). x").is_err());
        assert_eq!(parse_expr("x :: y").unwrap_err().span, Span::new(2, 4));
        assert!(parse_expr("x = y in x").is_err());
    }

    #[test]
    fn error_rendering() {
        let src = "x ::= y\nin \\. x";
        let err = parse_expr(src).unwrap_err();
        let rendered = err.render(src);
        assert!(rendered.contains("2:5"), "{rendered}");
    }

    #[test]
    fn roundtrip_is_stable() {
        for src in [
            "\\f x. f (f x)",
            "\\(f : (a -> b) -> c) (g : a -> b). f g",
            "(\\x. x x) (\\x. x x)",
            "k ::= \\x y. x in k a (k b c)",
            "f (x ::= a in x) b",
            "f \\y. x : a ::= y in x",
            "(f : a -> b) x",
        ] {
            let once = roundtrip(src);
            assert_eq!(roundtrip(&once), once, "source : {src}");
            let a = parse_expr(src).unwrap();
            let b = parse_expr(&once).unwrap();
            assert_eq!(a.to_string(), b.to_string());
        }
    }
}
