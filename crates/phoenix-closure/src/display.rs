//! Display of closure-converted programs:
//!
//! ```text
//! fun0 [] (x0):
//!   closure fun1 [x0]
//! fun1 [x0] (y1):
//!   x0
//! main:
//!   closure fun0 []
//! ```
//!
//! Each function shows its captures in brackets, then its parameters. Bodies
//! use the same layout as ANF blocks: one binding per line, the tail last.

use std::fmt::{self, Write};

use phoenix_anf::{Atom, Names, Var};

use crate::{Block, Comp, Program};

impl fmt::Display for Program<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = &self.names;
        for function in &self.functions {
            write!(f, "fun{} [", function.id.0)?;
            write_vars(f, names, function.captures.iter().map(|(v, _)| *v))?;
            write!(f, "] (")?;
            write_vars(f, names, function.params.iter().map(|(v, _)| *v))?;
            write!(f, "):")?;
            if let Some(name) = function.name {
                write!(f, "  -- {}", names.display(name))?;
            }
            writeln!(f)?;
            write_block(f, names, &function.body)?;
        }
        writeln!(f, "main:")?;
        write_block(f, names, &self.main)
    }
}

fn write_block(f: &mut impl Write, names: &Names<'_>, block: &Block<'_>) -> fmt::Result {
    for bind in &block.binds {
        write!(f, "  {} = ", names.display(bind.var))?;
        write_comp(f, names, &bind.comp)?;
        writeln!(f)?;
    }
    write!(f, "  ")?;
    write_comp(f, names, &block.tail)?;
    writeln!(f)
}

fn write_comp(f: &mut impl Write, names: &Names<'_>, comp: &Comp) -> fmt::Result {
    match comp {
        Comp::Atom(atom) => write_atom(f, names, atom),
        Comp::App { callee, args } => {
            write_atom(f, names, callee)?;
            for arg in args {
                write!(f, " ")?;
                write_atom(f, names, arg)?;
            }
            Ok(())
        }
        Comp::MakeClosure { fun, captures } => {
            write!(f, "closure fun{} [", fun.0)?;
            for (i, atom) in captures.iter().enumerate() {
                if i > 0 {
                    write!(f, " ")?;
                }
                write_atom(f, names, atom)?;
            }
            write!(f, "]")
        }
    }
}

fn write_atom(f: &mut impl Write, names: &Names<'_>, atom: &Atom) -> fmt::Result {
    match atom {
        Atom::Var(var) => write!(f, "{}", names.display(*var)),
    }
}

fn write_vars(f: &mut impl Write, names: &Names<'_>, vars: impl Iterator<Item = Var>) -> fmt::Result {
    for (i, var) in vars.enumerate() {
        if i > 0 {
            write!(f, " ")?;
        }
        write!(f, "{}", names.display(var))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use phoenix_anf::{Atom, Names};
    use phoenix_type::{Ty, TyVar};

    use crate::{Block, Comp, FunId, Function, Program};

    #[test]
    fn display_hand_built_program() {
        // The closure conversion of `\x. \y. x`.
        let mut names = Names::new();
        let x = names.fresh(Some("x"));
        let y = names.fresh(Some("y"));
        let a = Ty::Var(TyVar(0));
        let program = Program {
            functions: vec![
                Function {
                    id: FunId(0),
                    name: None,
                    captures: vec![],
                    params: vec![(x, a.clone())],
                    body: Block {
                        binds: vec![],
                        tail: Comp::MakeClosure {
                            fun: FunId(1),
                            captures: vec![Atom::Var(x)],
                        },
                    },
                },
                Function {
                    id: FunId(1),
                    name: None,
                    captures: vec![(x, a.clone())],
                    params: vec![(y, a)],
                    body: Block {
                        binds: vec![],
                        tail: Comp::Atom(Atom::Var(x)),
                    },
                },
            ],
            main: Block {
                binds: vec![],
                tail: Comp::MakeClosure {
                    fun: FunId(0),
                    captures: vec![],
                },
            },
            names,
        };
        assert_eq!(
            program.to_string(),
            "fun0 [] (x0):\n  closure fun1 [x0]\nfun1 [x0] (y1):\n  x0\nmain:\n  closure fun0 []\n"
        );
    }
}
