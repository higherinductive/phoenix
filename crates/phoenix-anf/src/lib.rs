//! A-normal form (ANF) of Phoenix programs.
//!
//! In ANF every argument of an application is an [`Atom`], and every
//! intermediate computation is named by a [`Bind`]. Nested `::=` bindings are
//! flattened into the [`Block`] that contains them, so a program becomes a
//! list of bindings followed by a computation in tail position. Only lambdas
//! open a new block, since their body runs when they are called.
//!
//! Every binder is alpha-renamed to a unique [`Var`]: shadowing disappears, and
//! later passes (closure conversion) never have to worry about capture.
//!
//! Only the IR and its plumbing are in place: the conversion itself
//! ([`convert`]) is left as `todo!()`.

mod convert;
mod display;
mod names;
mod scope;

use phoenix_type::{Ty, TypedExpr};

pub use convert::Converter;
pub use names::Names;
pub use scope::Scope;

/// Unique variable (after alpha-renaming).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Var(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Atom {
    Var(Var),
    // We may implement other atomic types afterwards
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comp<'a> {
    Atom(Atom),
    App { callee: Atom, args: Vec<Atom> },
    Lam(Box<Lambda<'a>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lambda<'a> {
    pub params: Vec<(Var, Ty<'a>)>,
    pub body: Block<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind<'a> {
    pub var: Var,
    pub ty: Ty<'a>,
    pub comp: Comp<'a>,
}

/// `let v1 = c1 in … let vn = cn in tail`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block<'a> {
    pub binds: Vec<Bind<'a>>,
    pub tail: Comp<'a>, // terminal component, e.g. `return_call` or `app_term`
}

/// A whole program in ANF: its top-level block and the source names of its
/// variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program<'a> {
    pub body: Block<'a>,
    pub names: Names<'a>,
}

/// Converts a typed program to ANF.
pub fn to_anf<'a>(expr: &TypedExpr<'a>) -> Program<'a> {
    let mut converter = Converter::new();
    let body = converter.block(expr);
    Program {
        body,
        names: converter.into_names(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_anf_src(src: &str) -> String {
        let expr = phoenix_parser::parse_expr(src).unwrap();
        let typed = phoenix_type::typecheck_expr(&expr).unwrap();
        to_anf(&typed).to_string()
    }

    /// Enable once the conversion is implemented.
    #[test]
    #[ignore = "ANF conversion not implemented yet"]
    fn nested_applications_are_named() {
        assert_eq!(
            to_anf_src("\\f g x. f (g x) x"),
            "\\f0 g1 x2 ->\n  t3 = g1 x2\n  f0 t3 x2\n"
        );
    }

    #[test]
    #[ignore = "ANF conversion not implemented yet"]
    fn lets_are_flattened_and_aliases_vanish() {
        assert_eq!(
            to_anf_src("\\a f g. x ::= (y ::= a in f y) in g x"),
            "\\a0 f1 g2 ->\n  x3 = f1 a0\n  g2 x3\n"
        );
    }
}
