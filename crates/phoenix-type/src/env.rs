//! Typing environment: maps variables in scope to their type schemes.

use std::collections::{BTreeSet, HashMap};

use crate::types::{Scheme, TyVar};

/// Persistent-by-clone environment. Each binder (lambda parameter, `::=`)
/// extends a copy of its parent with [`TypeEnv::extend`].
#[derive(Debug, Clone, Default)]
pub struct TypeEnv<'a> {
    bindings: HashMap<&'a str, Scheme<'a>>,
}

impl<'a> TypeEnv<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lookup(&self, name: &str) -> Option<&Scheme<'a>> {
        self.bindings.get(name)
    }

    /// Returns a new environment where `name` is bound to `scheme`,
    /// shadowing any previous binding.
    pub fn extend(&self, name: &'a str, scheme: Scheme<'a>) -> Self {
        let mut env = self.clone();
        env.bindings.insert(name, scheme);
        env
    }

    /// Free type variables of every scheme in scope (needed by generalization).
    pub fn free_vars(&self) -> BTreeSet<TyVar> {
        self.bindings.values().flat_map(Scheme::free_vars).collect()
    }
}
