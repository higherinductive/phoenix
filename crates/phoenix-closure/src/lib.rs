//! Closure conversion of ANF programs.
//!
//! Every lambda is lifted to a top-level [`Function`] that receives, besides
//! its parameters, the variables it captures from its surroundings. Where the
//! lambda stood, the program now builds a closure: a pointer to the function
//! and the values of its captures ([`Comp::MakeClosure`]).
//!
//! After this pass no code is nested anymore: a program is a flat list of
//! functions plus the `main` block, which is what the backend wants.
//!
//! Only the IR and its plumbing are in place: the conversion itself
//! ([`convert`]) is left as `todo!()`.

mod convert;
mod display;

use phoenix_anf::{Atom, Names, Var};
use phoenix_type::Ty;

pub use convert::{Converter, free_vars};

/// Index of a lifted function in [`Program::functions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comp {
    Atom(Atom),
    /// Call of an unknown closure. Known, saturated calls can later become
    /// direct calls to a [`FunId`].
    App { callee: Atom, args: Vec<Atom> },
    /// Allocates a closure for `fun`, capturing `captures` in the order of
    /// [`Function::captures`].
    MakeClosure { fun: FunId, captures: Vec<Atom> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind<'a> {
    pub var: Var,
    pub ty: Ty<'a>,
    pub comp: Comp,
}

/// `let v1 = c1 in … let vn = cn in tail`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block<'a> {
    pub binds: Vec<Bind<'a>>,
    pub tail: Comp,
}

/// A lifted lambda.
///
/// On entry, `captures[i]` is loaded from slot `i` of the closure's
/// environment, then `params` are bound to the arguments. Captured variables
/// keep the [`Var`] they had in the enclosing code: since every variable is
/// unique after ANF, this cannot clash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function<'a> {
    pub id: FunId,
    /// Variable the closure was bound to (`succ ::= \n f x. …`), if any:
    /// useful for debugging and for names in the readback.
    pub name: Option<Var>,
    pub captures: Vec<(Var, Ty<'a>)>,
    pub params: Vec<(Var, Ty<'a>)>,
    pub body: Block<'a>,
}

impl Function<'_> {
    pub fn arity(&self) -> usize {
        self.params.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program<'a> {
    /// Indexed by [`FunId`].
    pub functions: Vec<Function<'a>>,
    pub main: Block<'a>,
    pub names: Names<'a>,
}

impl<'a> Program<'a> {
    pub fn function(&self, id: FunId) -> &Function<'a> {
        &self.functions[id.0 as usize]
    }
}

/// Closure-converts an ANF program.
pub fn closure_convert<'a>(program: phoenix_anf::Program<'a>) -> Program<'a> {
    let mut converter = Converter::new();
    let main = converter.block(program.body);
    Program {
        functions: converter.into_functions(),
        main,
        names: program.names,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert_src(src: &str) -> String {
        let expr = phoenix_parser::parse_expr(src).unwrap();
        let typed = phoenix_type::typecheck_expr(&expr).unwrap();
        closure_convert(phoenix_anf::to_anf(&typed)).to_string()
    }

    /// Enable once ANF conversion and closure conversion are implemented.
    #[test]
    #[ignore = "conversions not implemented yet"]
    fn multi_param_lambda_captures_nothing() {
        assert_eq!(
            convert_src("\\x y. x"),
            "fun0 [] (x0 y1):\n  x0\nmain:\n  closure fun0 []\n"
        );
    }

    #[test]
    #[ignore = "conversions not implemented yet"]
    fn nested_lambda_captures_outer_param() {
        assert_eq!(
            convert_src("\\x. \\y. x"),
            "fun0 [] (x0):\n  closure fun1 [x0]\nfun1 [x0] (y1):\n  x0\nmain:\n  closure fun0 []\n"
        );
    }
}
