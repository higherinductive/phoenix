//! Closure conversion proper.
//!
//! The intended algorithm, one pass over the ANF tree:
//! - atoms and applications are copied as-is;
//! - a lambda `\params -> body` becomes `closure funN [fv1 … fvk]` where:
//!   1. `fv1 … fvk = free_vars(lambda)` (deterministic order);
//!   2. `funN` is reserved *before* converting `body`, so that a function
//!      always has a smaller id than the functions nested in it;
//!   3. `body` is converted recursively (nested lambdas are lifted too), and
//!      the function is defined with `captures = fv1 … fvk`.
//!
//! Free variables are easy to compute because ANF made every variable
//! unique: a variable is free in a lambda iff it is used in its body and not
//! bound by its parameters or by a binding of its body (including nested
//! lambdas' parameters and bindings).

use std::collections::{HashMap, HashSet};

use phoenix_anf::{Atom, Comp, Lambda, Var};
use phoenix_type::Ty;

use crate::{Bind, Block, FunId, Function};

/// Free variables of `lambda`, in order of first occurrence in its body.
pub fn free_vars(lambda: &Lambda<'_>) -> Vec<Var> {
    let mut bound: HashSet<Var> = lambda.params.iter().map(|(v, _)| *v).collect();
    let mut free = Vec::new();
    collect_block(&lambda.body, &mut bound, &mut free);
    free
}

fn collect_block(block: &phoenix_anf::Block<'_>, bound: &mut HashSet<Var>, free: &mut Vec<Var>) {
    for bind in &block.binds {
        collect_comp(&bind.comp, bound, free);
        bound.insert(bind.var);
    }
    collect_comp(&block.tail, bound, free);
}

fn collect_comp(comp: &Comp<'_>, bound: &mut HashSet<Var>, free: &mut Vec<Var>) {
    match comp {
        Comp::Atom(atom) => use_atom(atom, bound, free),
        Comp::App { callee, args } => {
            use_atom(callee, bound, free);
            for arg in args {
                use_atom(arg, bound, free);
            }
        }

        Comp::Lam(inner) => {
            bound.extend(inner.params.iter().map(|(v, _)| *v));
            collect_block(&inner.body, bound, free);
        }
    }
}

fn use_atom(atom: &Atom, bound: &HashSet<Var>, free: &mut Vec<Var>) {
    let Atom::Var(var) = atom;
    if !bound.contains(var) && !free.contains(var) {
        free.push(*var);
    }
}

#[derive(Debug, Default)]
pub struct Converter<'a> {
    /// Indexed by [`FunId`]; `None` while a reserved function's body is
    /// still being converted.
    functions: Vec<Option<Function<'a>>>,
    /// Type of every variable bound so far (parameters and bindings).
    types: HashMap<Var, Ty<'a>>,
}

impl<'a> Converter<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserves the id of a function whose body is not converted yet.
    pub fn reserve(&mut self) -> FunId {
        let id = FunId(self.functions.len() as u32);
        self.functions.push(None);
        id
    }

    /// Defines a function previously [reserved](Converter::reserve).
    pub fn define(&mut self, function: Function<'a>) {
        let slot = &mut self.functions[function.id.0 as usize];
        assert!(slot.is_none(), "function {:?} defined twice", function.id);
        *slot = Some(function);
    }

    /// Every lifted function, by id. Panics if one was reserved but never defined.
    pub fn into_functions(self) -> Vec<Function<'a>> {
        self.functions
            .into_iter()
            .enumerate()
            .map(|(i, f)| f.unwrap_or_else(|| panic!("function {i} reserved but never defined")))
            .collect()
    }

    /// Converts a block, lifting every lambda it contains.
    pub fn block(&mut self, block: phoenix_anf::Block<'a>) -> Block<'a> {
        let binds = block
            .binds
            .into_iter()
            .map(|bind| {
                let comp = self.comp(bind.comp, Some(bind.var));

                self.types.insert(bind.var, bind.ty.clone());
                Bind {
                    var: bind.var,
                    ty: bind.ty,
                    comp,
                }
            })
            .collect();
        let tail = self.comp(block.tail, None);
        Block { binds, tail }
    }

    fn comp(&mut self, comp: Comp<'a>, name: Option<Var>) -> crate::Comp {
        match comp {
            Comp::Atom(atom) => crate::Comp::Atom(atom),
            Comp::App { callee, args } => crate::Comp::App { callee, args },
            Comp::Lam(lambda) => {
                let (fun, captures) = self.lift(name, *lambda);
                crate::Comp::MakeClosure { fun, captures }
            }
        }
    }

    /// Lifts `lambda` to a top-level function and returns its id and its
    /// captures (the atoms the closure must be built with).
    ///
    /// `name` is the variable the closure is bound to, if any.
    pub fn lift(&mut self, name: Option<Var>, lambda: Lambda<'a>) -> (FunId, Vec<Atom>) {
        let id = self.reserve();
        let free = free_vars(&lambda);

        let captures = free.iter().map(|v| (*v, self.types[v].clone())).collect();
        for (var, ty) in &lambda.params {
            self.types.insert(*var, ty.clone());
        }

        let body = self.block(lambda.body);
        self.define(Function {
            id,
            name,
            captures,
            params: lambda.params,
            body,
        });

        (id, free.into_iter().map(Atom::Var).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Comp;

    fn trivial(id: FunId) -> Function<'static> {
        Function {
            id,
            name: None,
            captures: vec![],
            params: vec![],
            body: Block {
                binds: vec![],
                tail: Comp::Atom(Atom::Var(Var(0))),
            },
        }
    }

    #[test]
    fn reserve_then_define_in_any_order() {
        let mut c = Converter::new();
        let outer = c.reserve();
        let inner = c.reserve();
        c.define(trivial(inner));
        c.define(trivial(outer));
        let functions = c.into_functions();
        assert_eq!(functions[0].id, outer);
        assert_eq!(functions[1].id, inner);
    }

    #[test]
    #[should_panic(expected = "reserved but never defined")]
    fn undefined_function_is_a_bug() {
        let mut c = Converter::new();
        c.reserve();
        c.into_functions();
    }
}
