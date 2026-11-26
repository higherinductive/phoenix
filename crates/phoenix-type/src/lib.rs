//! Type inference for Phoenix's lambda calculus.
//!
//! The intended algorithm is Hindley–Milner (algorithm W style):
//! - lambda parameters get a fresh variable, or their annotation;
//! - applications unify the callee with `arg -> ?result`;
//! - `x ::= value in body` generalizes the type of `value` before
//!   checking `body`;
//! - `(e : T)` unifies the inferred type of `e` with `T`.
//!
//! Inference elaborates the source AST into a [typed AST](typed).

mod env;
mod error;
pub mod typed;
mod types;

use std::collections::{BTreeSet, HashMap};

use phoenix_ast::{Expr, PhoenixNode, Type, TypeNode};

pub use env::TypeEnv;
pub use error::{TypeError, TypeErrorKind};
pub use typed::{TypedExpr, TypedNode, TypedParam};
pub use types::{Scheme, Ty, TyVar};

pub type Result<'a, T> = std::result::Result<T, TypeError<'a>>;

/// Type-checks a closed expression in an empty environment and returns its
/// typed AST, with every type resolved against the final substitution.
pub fn typecheck_expr<'a>(expr: &Expr<'a>) -> Result<'a, TypedExpr<'a>> {
    let mut infer = Infer::new();
    let typed = infer.infer(&TypeEnv::new(), expr)?;
    Ok(infer.resolve(typed))
}

/// Infers the generalized type of a closed expression.
pub fn infer_expr<'a>(expr: &Expr<'a>) -> Result<'a, Scheme<'a>> {
    let typed = typecheck_expr(expr)?;
    Ok(Infer::new().generalize(&TypeEnv::new(), &typed.ty))
}

/// Inference state: fresh-variable supply and current substitution.
#[derive(Debug, Default)]
pub struct Infer<'a> {
    next_var: u32,
    /// Solved unification variables.
    subst: HashMap<TyVar, Ty<'a>>,
}

impl<'a> Infer<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a variable never handed out before.
    pub fn fresh(&mut self) -> Ty<'a> {
        let var = TyVar(self.next_var);
        self.next_var += 1;
        Ty::Var(var)
    }

    /// Replaces every solved variable of `ty` with its solution.
    pub fn apply(&self, ty: &Ty<'a>) -> Ty<'a> {
        match ty {
            Ty::Var(v) => match self.subst.get(v) {
                Some(solved) => self.apply(solved),
                None => ty.clone(),
            },
            Ty::Con(_) => ty.clone(),
            Ty::Arrow(from, to) => Ty::arrow(self.apply(from), self.apply(to)),
        }
    }

    /// Applies the current substitution to every type of `typed`.
    pub fn resolve(&self, typed: TypedExpr<'a>) -> TypedExpr<'a> {
        typed.map_types(&mut |ty| self.apply(ty))
    }

    /// Infers the type of `expr` under `env` and elaborates it into a typed AST.
    ///
    /// Types in the result are only resolved against the substitution as it
    /// was when each node was built; call [`Infer::resolve`] once inference is
    /// done.
    pub fn infer(&mut self, env: &TypeEnv<'a>, expr: &Expr<'a>) -> Result<'a, TypedExpr<'a>> {
        match &expr.node {
            PhoenixNode::Variable(var) => match env.lookup(var) {
                Some(scheme) => {
                    let ty = self.instantiate(scheme);
                    Ok(TypedExpr::new(TypedNode::Variable(var), ty, expr.span))
                }
                None => Err(TypeError::new(
                    TypeErrorKind::UnboundVariable(var),
                    expr.span,
                )),
            },

            // The annotation disappears from the typed AST: the inner
            // expression is kept, with the annotated type and the span of the
            // whole `(e : T)`.
            PhoenixNode::Annotation { expr: inner, ty } => {
                let typed = self.infer(env, inner)?;
                let expected = self.lower(ty);
                self.unify(&expected, &typed.ty, inner.span)?;
                Ok(TypedExpr::new(typed.node, expected, expr.span))
            }

            PhoenixNode::Application { callee, args } => {
                let callee = self.infer(env, callee)?;
                let args = args
                    .iter()
                    .map(|arg| self.infer(env, arg))
                    .collect::<Result<'a, Vec<_>>>()?;

                let return_ty = self.fresh();
                let arg_tys = args.iter().map(|arg| arg.ty.clone()).collect::<Vec<_>>();
                let arrow_ty = build_arrow(&arg_tys, return_ty.clone());
                self.unify(&callee.ty, &arrow_ty, expr.span)?;
                Ok(TypedExpr::new(
                    TypedNode::Application {
                        callee: Box::new(callee),
                        args,
                    },
                    return_ty,
                    expr.span,
                ))
            }

            // Non-recursive: `name` is not in scope inside `value`.
            PhoenixNode::Let {
                name,
                annotation,
                value,
                body,
            } => {
                let value = self.infer(env, value)?;
                if let Some(annotation) = annotation {
                    let expected = self.lower(annotation);
                    self.unify(&expected, &value.ty, value.span)?;
                }

                let scheme = self.generalize(env, &value.ty);
                let body = self.infer(&env.extend(name.node, scheme.clone()), body)?;
                let ty = body.ty.clone();
                Ok(TypedExpr::new(
                    TypedNode::Let {
                        name: name.clone(),
                        scheme,
                        value: Box::new(value),
                        body: Box::new(body),
                    },
                    ty,
                    expr.span,
                ))
            }

            PhoenixNode::Lambda { params, body } => {
                let mut body_env = env.clone();
                let mut typed_params = Vec::with_capacity(params.len());
                for param in params {
                    let ty = match &param.annotation {
                        Some(annotation) => self.lower(annotation),
                        None => self.fresh(),
                    };
                    body_env = body_env.extend(param.name.node, Scheme::mono(ty.clone()));
                    typed_params.push(TypedParam {
                        name: param.name.clone(),
                        ty,
                    });
                }

                let body = self.infer(&body_env, body)?;
                let param_tys = typed_params
                    .iter()
                    .map(|p| p.ty.clone())
                    .collect::<Vec<_>>();
                let ty = build_arrow(&param_tys, body.ty.clone());
                Ok(TypedExpr::new(
                    TypedNode::Lambda {
                        params: typed_params,
                        body: Box::new(body),
                    },
                    ty,
                    expr.span,
                ))
            }
        }
    }

    /// Makes `expected` and `found` equal by extending the substitution.
    /// `span` locates the error if they cannot be unified.
    pub fn unify(
        &mut self,
        expected: &Ty<'a>,
        found: &Ty<'a>,
        span: phoenix_ast::Span,
    ) -> Result<'a, ()> {
        // Work on fully resolved types so that solved variables are never overwritten.
        let (expected, found) = (self.apply(expected), self.apply(found));
        match (&expected, &found) {
            (Ty::Var(v1), Ty::Var(v2)) if v1 == v2 => Ok(()),

            (Ty::Var(var), ty) | (ty, Ty::Var(var)) => self.bind(*var, ty, span),

            (Ty::Con(c1), Ty::Con(c2)) if c1 == c2 => Ok(()),

            (Ty::Arrow(a1, r1), Ty::Arrow(a2, r2)) => {
                self.unify(a1, a2, span)?;
                self.unify(r1, r2, span)
            }

            _ => Err(TypeError::new(
                TypeErrorKind::Mismatch { expected, found },
                span,
            )),
        }
    }

    /// Solves `var := ty`, after the occurs check. `ty` must already be resolved.
    fn bind(&mut self, var: TyVar, ty: &Ty<'a>, span: phoenix_ast::Span) -> Result<'a, ()> {
        if ty.free_vars().contains(&var) {
            return Err(TypeError::new(
                TypeErrorKind::InfiniteType {
                    var,
                    ty: ty.clone(),
                },
                span,
            ));
        }
        self.subst.insert(var, ty.clone());
        Ok(())
    }

    /// Quantifies the variables of `ty` that are not free in `env`.
    pub fn generalize(&self, env: &TypeEnv<'a>, ty: &Ty<'a>) -> Scheme<'a> {
        let ty = self.apply(ty);
        // The environment's free variables must be resolved too: `?0` in the
        // environment may have been solved to a type mentioning `?3`.
        let env_vars: BTreeSet<TyVar> = env
            .free_vars()
            .into_iter()
            .flat_map(|v| self.apply(&Ty::Var(v)).free_vars())
            .collect();
        Scheme {
            vars: ty.free_vars().difference(&env_vars).copied().collect(),
            ty,
        }
    }

    /// Replaces the quantified variables of `scheme` with fresh ones.
    pub fn instantiate(&mut self, scheme: &Scheme<'a>) -> Ty<'a> {
        // A local renaming: the quantified variables must not be solved in
        // `self.subst`, or every other instantiation would share them.
        let renaming: HashMap<TyVar, Ty<'a>> =
            scheme.vars.iter().map(|&var| (var, self.fresh())).collect();
        rename(&self.apply(&scheme.ty), &renaming)
    }

    /// Converts a surface type annotation into an internal type.
    ///
    /// Still to decide: whether lowercase names (`a`) are fresh/rigid type
    /// variables or ordinary constructors.
    pub fn lower(&mut self, ty: &Type<'a>) -> Ty<'a> {
        match &ty.node {
            TypeNode::Name(name) => Ty::Con(name),
            TypeNode::Arrow(l, r) => Ty::arrow(self.lower(l), self.lower(r)),
        }
    }
}

/// Builds `a1 -> a2 -> ... -> ret`.
fn build_arrow<'a>(args: &[Ty<'a>], ret: Ty<'a>) -> Ty<'a> {
    args.iter()
        .rev()
        .fold(ret, |acc, arg| Ty::arrow(arg.clone(), acc))
}

/// Replaces the variables of `ty` found in `renaming`.
fn rename<'a>(ty: &Ty<'a>, renaming: &HashMap<TyVar, Ty<'a>>) -> Ty<'a> {
    match ty {
        Ty::Var(v) => renaming.get(v).cloned().unwrap_or_else(|| ty.clone()),
        Ty::Con(_) => ty.clone(),
        Ty::Arrow(from, to) => Ty::arrow(rename(from, renaming), rename(to, renaming)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_vars_are_distinct() {
        let mut infer = Infer::new();
        assert_ne!(infer.fresh(), infer.fresh());
    }

    #[test]
    fn apply_follows_substitution_chains() {
        let mut infer = Infer::new();
        let (a, b) = (infer.fresh(), infer.fresh());
        infer.subst.insert(TyVar(0), Ty::arrow(b.clone(), b));
        infer.subst.insert(TyVar(1), Ty::Con("Int"));
        assert_eq!(infer.apply(&a).to_string(), "Int -> Int");
    }

    #[test]
    fn display() {
        let ty = Ty::arrow(
            Ty::arrow(Ty::Var(TyVar(0)), Ty::Con("Int")),
            Ty::Var(TyVar(0)),
        );
        assert_eq!(ty.to_string(), "(?a -> Int) -> ?a");
        let scheme = Scheme {
            vars: vec![TyVar(0)],
            ty,
        };
        assert_eq!(scheme.to_string(), "forall ?a. (?a -> Int) -> ?a");
        assert!(scheme.free_vars().is_empty());
    }

    #[test]
    fn env_shadowing_and_free_vars() {
        let env = TypeEnv::new()
            .extend("x", Scheme::mono(Ty::Var(TyVar(0))))
            .extend("x", Scheme::mono(Ty::Var(TyVar(1))));
        assert_eq!(env.lookup("x"), Some(&Scheme::mono(Ty::Var(TyVar(1)))));
        assert_eq!(
            env.free_vars().into_iter().collect::<Vec<_>>(),
            vec![TyVar(1)]
        );
    }

    /// Infers `src` and renumbers its variables `?a, ?b, ...` in order of appearance.
    fn infer_src(src: &str) -> std::result::Result<String, String> {
        let expr = phoenix_parser::parse_expr(src).unwrap();
        let scheme = infer_expr(&expr).map_err(|e| e.kind.to_string())?;
        let mut order = Vec::new();
        collect_in_order(&scheme.ty, &mut order);
        let renaming = order
            .iter()
            .enumerate()
            .map(|(i, &v)| (v, Ty::Var(TyVar(i as u32))))
            .collect();
        let vars = scheme
            .vars
            .iter()
            .map(|v| TyVar(order.iter().position(|o| o == v).unwrap() as u32))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Scheme {
            vars,
            ty: rename(&scheme.ty, &renaming),
        }
        .to_string())
    }

    fn collect_in_order(ty: &Ty<'_>, order: &mut Vec<TyVar>) {
        match ty {
            Ty::Var(v) if !order.contains(v) => order.push(*v),
            Ty::Arrow(from, to) => {
                collect_in_order(from, order);
                collect_in_order(to, order);
            }
            _ => {}
        }
    }

    #[test]
    fn infer_identity() {
        assert_eq!(infer_src("\\x. x").unwrap(), "forall ?a. ?a -> ?a");
    }

    #[test]
    fn infer_application_order() {
        assert_eq!(
            infer_src("\\f x y. f x y").unwrap(),
            "forall ?a ?b ?c. (?a -> ?b -> ?c) -> ?a -> ?b -> ?c"
        );
    }

    #[test]
    fn infer_example_const() {
        assert_eq!(
            infer_src("const ::= \\x y. y in \\x. const x x").unwrap(),
            "forall ?a. ?a -> ?a"
        );
    }

    #[test]
    fn let_is_polymorphic() {
        assert_eq!(
            infer_src("id ::= \\x. x in id id").unwrap(),
            "forall ?a. ?a -> ?a"
        );
        // Lambda-bound variables are not generalized.
        assert!(infer_src("(\\id. id id) (\\x. x)").is_err());
    }

    #[test]
    fn annotations() {
        assert_eq!(infer_src("\\(x : Int). x").unwrap(), "Int -> Int");
        assert_eq!(infer_src("(\\x. x : Int -> Int)").unwrap(), "Int -> Int");
        assert_eq!(
            infer_src("f : Int -> Int ::= \\x. x in f").unwrap(),
            "Int -> Int"
        );
        assert_eq!(
            infer_src("(\\(x : Int). x : Bool -> Bool)").unwrap_err(),
            "Type mismatch: expected `Bool`, found `Int`"
        );
    }

    fn typecheck_src(src: &str) -> TypedExpr<'_> {
        typecheck_expr(&phoenix_parser::parse_expr(src).unwrap()).unwrap()
    }

    #[test]
    fn typed_ast_annotates_binders() {
        assert_eq!(
            typecheck_src("const ::= \\x y. y in \\x. const x x").to_string(),
            "const : forall ?a ?b. ?a -> ?b -> ?b ::= \\(x : ?a) (y : ?b). y in\n\\(x : ?f). const x x"
        );
    }

    #[test]
    fn typed_ast_types_are_resolved() {
        // `x`'s type is only known after checking the application in the body.
        let typed = typecheck_src("\\f x. f (x : Int)");
        let TypedNode::Lambda { params, .. } = &typed.node else {
            panic!()
        };
        assert_eq!(params[1].ty, Ty::Con("Int"));
        assert_eq!(params[0].ty.to_string(), "Int -> ?c");
        assert_eq!(typed.ty.to_string(), "(Int -> ?c) -> Int -> ?c");

        // Each occurrence of a polymorphic variable gets its own instance.
        let typed = typecheck_src("id ::= \\x. x in id id");
        let TypedNode::Let { body, .. } = &typed.node else {
            panic!()
        };
        let TypedNode::Application { callee, args } = &body.node else {
            panic!()
        };
        assert_eq!(callee.ty.to_string(), "(?c -> ?c) -> ?c -> ?c");
        assert_eq!(args[0].ty.to_string(), "?c -> ?c");
    }

    #[test]
    fn typed_ast_drops_annotations() {
        let typed = typecheck_src("(\\x. x : Int -> Int)");
        assert!(matches!(typed.node, TypedNode::Lambda { .. }));
        assert_eq!(typed.span, phoenix_ast::Span::new(0, 20));
        assert_eq!(typed.to_string(), "\\(x : Int). x");
    }

    #[test]
    fn errors() {
        assert_eq!(infer_src("\\x. y").unwrap_err(), "Unbound variable `y`");
        assert!(
            infer_src("\\x. x x")
                .unwrap_err()
                .starts_with("Infinite type")
        );
        // `::=` is not recursive.
        assert_eq!(
            infer_src("f ::= \\x. f x in f").unwrap_err(),
            "Unbound variable `f`"
        );
    }
}
