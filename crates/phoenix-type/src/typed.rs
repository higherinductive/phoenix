//! Typed AST: the output of inference.
//!
//! It mirrors [`phoenix_ast::PhoenixNode`], except that:
//! - every node carries its inferred [`Ty`];
//! - every lambda parameter carries its type, annotated or inferred;
//! - every `::=` binding carries the [`Scheme`] it was generalized to;
//! - `(e : T)` annotations are gone: once checked, they are just `e` with type `T`.
//!
//! [`crate::typecheck_expr`] returns a tree whose types are fully resolved
//! against the final substitution. Variables still present in it are the
//! ones inference left unconstrained.

use std::fmt;

use phoenix_ast::{Ident, Span};

use crate::types::{Scheme, Ty};

/// An expression with its type and its position in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedExpr<'a> {
    pub node: TypedNode<'a>,
    pub ty: Ty<'a>,
    pub span: Span,
}

/// A lambda parameter with its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedParam<'a> {
    pub name: Ident<'a>,
    pub ty: Ty<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedNode<'a> {
    /// Variable. The node's type is the instance of the variable's scheme
    /// used at this occurrence.
    Variable(&'a str),

    Lambda {
        params: Vec<TypedParam<'a>>,
        body: Box<TypedExpr<'a>>,
    },

    Application {
        callee: Box<TypedExpr<'a>>,
        args: Vec<TypedExpr<'a>>,
    },

    Let {
        name: Ident<'a>,
        /// Generalized type of `value`, as seen by `body`.
        scheme: Scheme<'a>,
        value: Box<TypedExpr<'a>>,
        body: Box<TypedExpr<'a>>,
    },
}

impl<'a> TypedExpr<'a> {
    pub fn new(node: TypedNode<'a>, ty: Ty<'a>, span: Span) -> Self {
        Self { node, ty, span }
    }

    /// Rewrites every type in the tree (node types, parameter types and the
    /// body of let schemes) with `f`.
    pub fn map_types(self, f: &mut impl FnMut(&Ty<'a>) -> Ty<'a>) -> Self {
        let node = match self.node {
            TypedNode::Variable(name) => TypedNode::Variable(name),
            TypedNode::Lambda { params, body } => TypedNode::Lambda {
                params: params
                    .into_iter()
                    .map(|p| TypedParam { ty: f(&p.ty), name: p.name })
                    .collect(),
                body: Box::new(body.map_types(f)),
            },
            TypedNode::Application { callee, args } => TypedNode::Application {
                callee: Box::new(callee.map_types(f)),
                args: args.into_iter().map(|arg| arg.map_types(f)).collect(),
            },
            TypedNode::Let { name, scheme, value, body } => TypedNode::Let {
                name,
                scheme: Scheme { ty: f(&scheme.ty), vars: scheme.vars },
                value: Box::new(value.map_types(f)),
                body: Box::new(body.map_types(f)),
            },
        };
        TypedExpr::new(node, f(&self.ty), self.span)
    }
}

// ---------------------------------------------------------------------------
// Display: the source syntax with every binder annotated, e.g.
// `const : forall ?0 ?1. ?0 -> ?1 -> ?1 ::= \(x : ?0) (y : ?1). y in ...`.
// Meant for debugging: schemes (`forall`) are not valid source syntax.
// ---------------------------------------------------------------------------

impl fmt::Display for TypedParam<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} : {})", self.name.node, self.ty)
    }
}

impl fmt::Display for TypedExpr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.node {
            TypedNode::Variable(name) => write!(f, "{name}"),
            TypedNode::Lambda { params, body } => {
                write!(f, "\\")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ". {body}")
            }
            TypedNode::Application { callee, args } => {
                fmt_atom(callee, f)?;
                let last = args.len() - 1;
                for (i, arg) in args.iter().enumerate() {
                    write!(f, " ")?;
                    if i == last && matches!(arg.node, TypedNode::Lambda { .. }) {
                        write!(f, "{arg}")?;
                    } else {
                        fmt_atom(arg, f)?;
                    }
                }
                Ok(())
            }
            TypedNode::Let { name, scheme, value, body } => {
                write!(f, "{} : {scheme} ::= {value} in\n{body}", name.node)
            }
        }
    }
}

fn fmt_atom(e: &TypedExpr<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match e.node {
        TypedNode::Variable(_) => write!(f, "{e}"),
        _ => write!(f, "({e})"),
    }
}
