use std::fmt;

use phoenix_ast::Span;

use crate::types::{Ty, TyVar};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeErrorKind<'a> {
    /// A variable is used but not bound.
    UnboundVariable(&'a str),
    /// Two types could not be unified.
    Mismatch { expected: Ty<'a>, found: Ty<'a> },
    /// Occurs check failure: `var` would have to contain itself (`?0 = ?0 -> a`).
    InfiniteType { var: TyVar, ty: Ty<'a> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeError<'a> {
    pub kind: TypeErrorKind<'a>,
    pub span: Span,
}

impl<'a> TypeError<'a> {
    pub fn new(kind: TypeErrorKind<'a>, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for TypeErrorKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeErrorKind::UnboundVariable(name) => write!(f, "Unbound variable `{name}`"),
            TypeErrorKind::Mismatch { expected, found } => {
                write!(f, "Type mismatch: expected `{expected}`, found `{found}`")
            }
            TypeErrorKind::InfiniteType { var, ty } => {
                write!(f, "Infinite type: `{var}` occurs in `{ty}`")
            }
        }
    }
}

impl fmt::Display for TypeError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (bytes {}..{})", self.kind, self.span.start, self.span.end)
    }
}

impl std::error::Error for TypeError<'_> {}
