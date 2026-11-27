//! Display of ANF programs, one binding per line:
//!
//! ```text
//! succ6 = \n7 f8 x9 ->
//!   t10 = n7 f8 x9
//!   f8 t10
//! t11 = succ6 zero3
//! add12 t11 t11
//! ```
//!
//! The last line of each block is its tail computation. Lambda bodies are
//! indented under the lambda.

use std::fmt::{self, Write};

use crate::{Atom, Block, Comp, Names, Program};

impl fmt::Display for Program<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_block(f, &self.names, &self.body, 0)
    }
}

fn write_block(f: &mut impl Write, names: &Names<'_>, block: &Block<'_>, depth: usize) -> fmt::Result {
    for bind in &block.binds {
        indent(f, depth)?;
        write!(f, "{} = ", names.display(bind.var))?;
        write_comp(f, names, &bind.comp, depth)?;
    }
    indent(f, depth)?;
    write_comp(f, names, &block.tail, depth)
}

/// Writes `comp` and the end of its line (lambdas also write their body).
fn write_comp(f: &mut impl Write, names: &Names<'_>, comp: &Comp<'_>, depth: usize) -> fmt::Result {
    match comp {
        Comp::Atom(atom) => {
            write_atom(f, names, atom)?;
            writeln!(f)
        }
        Comp::App { callee, args } => {
            write_atom(f, names, callee)?;
            for arg in args {
                write!(f, " ")?;
                write_atom(f, names, arg)?;
            }
            writeln!(f)
        }
        Comp::Lam(lambda) => {
            write!(f, "\\")?;
            for (i, (param, _)) in lambda.params.iter().enumerate() {
                if i > 0 {
                    write!(f, " ")?;
                }
                write!(f, "{}", names.display(*param))?;
            }
            writeln!(f, " ->")?;
            write_block(f, names, &lambda.body, depth + 1)
        }
    }
}

fn write_atom(f: &mut impl Write, names: &Names<'_>, atom: &Atom) -> fmt::Result {
    match atom {
        Atom::Var(var) => write!(f, "{}", names.display(*var)),
    }
}

fn indent(f: &mut impl Write, depth: usize) -> fmt::Result {
    for _ in 0..depth {
        f.write_str("  ")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use phoenix_type::{Ty, TyVar};

    use crate::{Atom, Bind, Block, Comp, Lambda, Names, Program};

    #[test]
    fn display_hand_built_program() {
        // f0 = \x1 -> x1
        // t2 = f0 f0
        // t2 f0
        let mut names = Names::new();
        let f = names.fresh(Some("f"));
        let x = names.fresh(Some("x"));
        let t = names.fresh(None);
        let a = Ty::Var(TyVar(0));
        let program = Program {
            body: Block {
                binds: vec![
                    Bind {
                        var: f,
                        ty: Ty::arrow(a.clone(), a.clone()),
                        comp: Comp::Lam(Box::new(Lambda {
                            params: vec![(x, a.clone())],
                            body: Block {
                                binds: vec![],
                                tail: Comp::Atom(Atom::Var(x)),
                            },
                        })),
                    },
                    Bind {
                        var: t,
                        ty: Ty::arrow(a.clone(), a),
                        comp: Comp::App {
                            callee: Atom::Var(f),
                            args: vec![Atom::Var(f)],
                        },
                    },
                ],
                tail: Comp::App {
                    callee: Atom::Var(t),
                    args: vec![Atom::Var(f)],
                },
            },
            names,
        };
        assert_eq!(program.to_string(), "f0 = \\x1 ->\n  x1\nt2 = f0 f0\nt2 f0\n");
    }
}
