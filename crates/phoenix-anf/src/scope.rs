//! Maps source names in scope to what they stand for in ANF.

use std::collections::HashMap;

use crate::Atom;

/// Stack of scopes. A name maps to an [`Atom`] rather than a [`crate::Var`] so
/// that `x ::= y in …` can make `x` a plain alias of `y`, without emitting a
/// binding.
#[derive(Debug, Clone)]
pub struct Scope<'a> {
    frames: Vec<HashMap<&'a str, Atom>>,
}

impl Default for Scope<'_> {
    fn default() -> Self {
        Self {
            frames: vec![HashMap::new()],
        }
    }
}

impl<'a> Scope<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a scope, closed by [`Scope::pop`].
    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        assert!(self.frames.len() > 1, "popped the outermost scope");
        self.frames.pop();
    }

    /// Binds `name` in the innermost scope, shadowing outer bindings.
    pub fn insert(&mut self, name: &'a str, atom: Atom) {
        self.frames
            .last_mut()
            .expect("there is always a scope")
            .insert(name, atom);
    }

    pub fn lookup(&self, name: &str) -> Option<Atom> {
        self.frames.iter().rev().find_map(|frame| frame.get(name).copied())
    }
}
