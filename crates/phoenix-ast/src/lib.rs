use std::fmt;

/// Byte interval `[start, end)` in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the smallest span that covers both `self` and `other`.
    pub fn merge(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// Localized value with a [`Span`] for error reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

/// Localized identifier.
pub type Ident<'a> = Spanned<&'a str>;

/// Localized expression.
pub type Expr<'a> = Spanned<PhoenixNode<'a>>;

/// Localized type.
pub type Type<'a> = Spanned<TypeNode<'a>>;

/// Lambda parameter: `x` or `(x : T)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param<'a> {
    pub name: Ident<'a>,
    pub annotation: Option<Type<'a>>,
}

/// Lambda-calculus expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhoenixNode<'a> {
    /// Variable: `x`
    Variable(&'a str),

    /// Abstraction: `\x (y : T). body`
    ///
    /// Multiple parameters are kept as-is (rather than desugared into
    /// nested lambdas) to preserve the source structure.
    Lambda {
        params: Vec<Param<'a>>,
        body: Box<Expr<'a>>,
    },

    /// Application: `f a b` (left-associative, non-empty `args`)
    Application {
        callee: Box<Expr<'a>>,
        args: Vec<Expr<'a>>,
    },

    /// Local binding: `x : T ::= value in body` (annotation optional)
    ///
    /// Kept distinct from `(\x. body) value` to allow later
    /// generalization to Hindley–Milner.
    Let {
        name: Ident<'a>,
        annotation: Option<Type<'a>>,
        value: Box<Expr<'a>>,
        body: Box<Expr<'a>>,
    },

    /// Explicit type annotation: `(e : T)`
    Annotation { expr: Box<Expr<'a>>, ty: Type<'a> },
}

/// Type syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeNode<'a> {
    /// Named type or type variable: `Int`, `a`
    Name(&'a str),

    /// Function type: `A -> B` (right-associative)
    Arrow(Box<Type<'a>>, Box<Type<'a>>),
}

impl fmt::Display for Param<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.annotation {
            None => write!(f, "{}", self.name.node),
            Some(ty) => write!(f, "({} : {})", self.name.node, ty),
        }
    }
}

impl fmt::Display for PhoenixNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhoenixNode::Variable(name) => write!(f, "{name}"),
            PhoenixNode::Lambda { params, body } => {
                write!(f, "\\")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ". {}", body.node)
            }
            PhoenixNode::Application { callee, args } => {
                fmt_atom(&callee.node, f)?;
                let last = args.len() - 1;
                for (i, arg) in args.iter().enumerate() {
                    write!(f, " ")?;
                    // A trailing lambda needs no parentheses; a trailing
                    // binding does, since `f x ::= ...` does not parse.
                    if i == last && matches!(arg.node, PhoenixNode::Lambda { .. }) {
                        write!(f, "{}", arg.node)?;
                    } else {
                        fmt_atom(&arg.node, f)?;
                    }
                }
                Ok(())
            }
            PhoenixNode::Let {
                name,
                annotation,
                value,
                body,
            } => {
                write!(f, "{}", name.node)?;
                if let Some(ty) = annotation {
                    write!(f, " : {ty}")?;
                }
                write!(f, " ::= {} in {}", value.node, body.node)
            }
            PhoenixNode::Annotation { expr, ty } => write!(f, "({} : {})", expr.node, ty),
        }
    }
}

fn fmt_atom(e: &PhoenixNode<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match e {
        PhoenixNode::Variable(_) | PhoenixNode::Annotation { .. } => write!(f, "{e}"),
        _ => write!(f, "({e})"),
    }
}

impl fmt::Display for TypeNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeNode::Name(name) => write!(f, "{name}"),
            TypeNode::Arrow(from, to) => {
                match from.node {
                    TypeNode::Arrow(..) => write!(f, "({})", from.node)?,
                    TypeNode::Name(_) => write!(f, "{}", from.node)?,
                }
                write!(f, " -> {}", to.node)
            }
        }
    }
}

impl<T: fmt::Display> fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.node.fmt(f)
    }
}
