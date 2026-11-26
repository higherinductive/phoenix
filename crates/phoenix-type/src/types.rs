//! Internal representation of types, distinct from the surface syntax
//! ([`phoenix_ast::TypeNode`]): it has unification variables and no spans.

use std::collections::BTreeSet;
use std::fmt;

/// Unification variable, created fresh during inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TyVar(pub u32);

/// A monotype.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty<'a> {
    /// Unification variable: `?0`, `?1`, ...
    Var(TyVar),
    /// Named type constructor: `Int`, `Bool`, ...
    Con(&'a str),
    /// Function type: `A -> B`
    Arrow(Box<Ty<'a>>, Box<Ty<'a>>),
}

impl<'a> Ty<'a> {
    pub fn arrow(from: Ty<'a>, to: Ty<'a>) -> Self {
        Ty::Arrow(Box::new(from), Box::new(to))
    }

    /// Unification variables occurring in this type.
    pub fn free_vars(&self) -> BTreeSet<TyVar> {
        let mut vars = BTreeSet::new();
        self.collect_free_vars(&mut vars);
        vars
    }

    fn collect_free_vars(&self, vars: &mut BTreeSet<TyVar>) {
        match self {
            Ty::Var(v) => {
                vars.insert(*v);
            }
            Ty::Con(_) => {}
            Ty::Arrow(from, to) => {
                from.collect_free_vars(vars);
                to.collect_free_vars(vars);
            }
        }
    }
}

/// A polytype `forall vars. ty`, as bound by `x ::= value in body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme<'a> {
    pub vars: Vec<TyVar>,
    pub ty: Ty<'a>,
}

impl<'a> Scheme<'a> {
    /// A scheme with no quantified variables.
    pub fn mono(ty: Ty<'a>) -> Self {
        Self {
            vars: Vec::new(),
            ty,
        }
    }

    /// Free variables of the scheme (those of `ty` that are not quantified).
    pub fn free_vars(&self) -> BTreeSet<TyVar> {
        let mut vars = self.ty.free_vars();
        for v in &self.vars {
            vars.remove(v);
        }
        vars
    }
}

impl fmt::Display for TyVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write corresponding character from ASCII table.
        // Should add numbers for large indices.
        if self.0 < 26 {
            write!(f, "?{}", (self.0 as u8 + 97) as char)
        } else {
            write!(f, "?{}", self.0)
        }
    }
}

impl fmt::Display for Ty<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Var(v) => write!(f, "{v}"),
            Ty::Con(name) => write!(f, "{name}"),
            Ty::Arrow(from, to) => {
                match **from {
                    Ty::Arrow(..) => write!(f, "({from})")?,
                    _ => write!(f, "{from}")?,
                }
                write!(f, " -> {to}")
            }
        }
    }
}

impl fmt::Display for Scheme<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.vars.is_empty() {
            write!(f, "forall")?;
            for v in &self.vars {
                write!(f, " {v}")?;
            }
            write!(f, ". ")?;
        }
        write!(f, "{}", self.ty)
    }
}
