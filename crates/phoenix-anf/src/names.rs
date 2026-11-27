//! Supply of fresh variables, remembering the source name each one came from.

use std::fmt;

use crate::Var;

/// Table indexed by [`Var`]: `hints[v]` is the source name of `v`, or `None`
/// for a temporary introduced by the conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names<'a> {
    hints: Vec<Option<&'a str>>,
}

impl<'a> Names<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a variable never handed out before.
    pub fn fresh(&mut self, hint: Option<&'a str>) -> Var {
        let var = Var(self.hints.len() as u32);
        self.hints.push(hint);
        var
    }

    /// Source name of `var`, if it has one.
    pub fn hint(&self, var: Var) -> Option<&'a str> {
        self.hints.get(var.0 as usize).copied().flatten()
    }

    /// Number of variables handed out so far.
    pub fn len(&self) -> usize {
        self.hints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hints.is_empty()
    }

    /// Displays `var` as its source name followed by its number (`x3`), or
    /// `t3` for a temporary. The number keeps names unique.
    pub fn display(&self, var: Var) -> VarDisplay<'_, 'a> {
        VarDisplay { names: self, var }
    }
}

pub struct VarDisplay<'n, 'a> {
    names: &'n Names<'a>,
    var: Var,
}

impl fmt::Display for VarDisplay<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hint = self.names.hint(self.var).unwrap_or("t");
        write!(f, "{hint}{}", self.var.0)
    }
}
