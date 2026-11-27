//! Conversion from the typed AST to ANF.
//!
//! The intended algorithm:
//! - [`Converter::block`] opens a new block: it is used for the whole program
//!   and for lambda bodies;
//! - [`Converter::comp`] converts an expression to a [`Comp`], pushing the
//!   bindings it needs into `out`;
//! - [`Converter::atom`] does the same, then names the result with a fresh
//!   temporary unless it is already atomic.
//!
//! Cases of [`Converter::comp`]:
//! - `Variable(x)`: the atom `x` stands for in the [`Scope`];
//! - `Application`: atomize the callee, then the arguments from left to right
//!   (this fixes the evaluation order);
//! - `Lambda`: fresh variables for the parameters, and a *new* block for
//!   the body;
//! - `Let`: convert the value into the *same* `out`, bind the name (or make it
//!   an alias if the value is atomic), then continue with the body in the
//!   same `out` too: this is what flattens nested lets.

use phoenix_type::{TypedExpr, TypedNode};

use crate::{Atom, Bind, Block, Comp, Lambda, Names, Scope, Var};

#[derive(Debug, Default)]
pub struct Converter<'a> {
    names: Names<'a>,
    scope: Scope<'a>,
}

impl<'a> Converter<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Gives back the variable table once conversion is done.
    pub fn into_names(self) -> Names<'a> {
        self.names
    }

    /// Fresh variable for the source binder `name`, bound in the current scope.
    pub fn bind_name(&mut self, name: &'a str) -> Var {
        let var = self.names.fresh(Some(name));
        self.scope.insert(name, Atom::Var(var));
        var
    }

    /// Fresh temporary, with no source name.
    pub fn temp(&mut self) -> Var {
        self.names.fresh(None)
    }

    /// Converts `expr` into a block of its own.
    pub fn block(&mut self, expr: &TypedExpr<'a>) -> Block<'a> {
        let mut binds = Vec::new();
        let tail = self.comp(expr, &mut binds);
        Block { binds, tail }
    }

    /// Converts `expr` into an atom, naming it if needed.
    pub fn atom(&mut self, expr: &TypedExpr<'a>, out: &mut Vec<Bind<'a>>) -> Atom {
        match self.comp(expr, out) {
            Comp::Atom(atom) => atom,
            c => {
                let var = self.temp();
                out.push(Bind {
                    var,
                    ty: expr.ty.clone(),
                    comp: c,
                });
                Atom::Var(var)
            }
        }
    }

    /// Converts `expr` into a computation, pushing the bindings it needs into `out`.
    pub fn comp(&mut self, expr: &TypedExpr<'a>, out: &mut Vec<Bind<'a>>) -> Comp<'a> {
        match &expr.node {
            TypedNode::Variable(v) => Comp::Atom(self.scope.lookup(v).unwrap()),

            TypedNode::Application { callee, args } => {
                let callee = self.atom(callee, out);
                let args = args
                    .iter()
                    .map(|arg| self.atom(arg, out))
                    .collect::<Vec<_>>();
                Comp::App { callee, args }
            }

            TypedNode::Lambda { params, body } => {
                self.scope.push();
                let params = params
                    .iter()
                    .map(|param| (self.bind_name(param.name.node), param.ty.clone()))
                    .collect::<Vec<_>>();
                let body = self.block(body);

                self.scope.pop();

                Comp::Lam(Box::new(Lambda { body, params }))
            }

            TypedNode::Let {
                name,
                scheme,
                value,
                body,
            } => {
                let value = self.comp(value, out);

                self.scope.push();

                match value {
                    Comp::Atom(atom) => self.scope.insert(name.node, atom),
                    c => {
                        let var = self.bind_name(name.node);

                        out.push(Bind {
                            var,
                            ty: scheme.ty.clone(),
                            comp: c,
                        })
                    }
                }

                let tail = self.comp(body, out);
                self.scope.pop();

                tail
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binders_get_unique_vars_and_shadow() {
        let mut c = Converter::new();
        let x0 = c.bind_name("x");
        c.scope.push();
        let x1 = c.bind_name("x");
        assert_ne!(x0, x1);
        assert_eq!(c.scope.lookup("x"), Some(Atom::Var(x1)));
        c.scope.pop();
        assert_eq!(c.scope.lookup("x"), Some(Atom::Var(x0)));
        assert_eq!(c.scope.lookup("y"), None);
    }

    #[test]
    fn temporaries_have_no_source_name() {
        let mut c = Converter::new();
        let t = c.temp();
        let names = c.into_names();
        assert_eq!(names.hint(t), None);
        assert_eq!(names.display(t).to_string(), "t0");
    }
}
