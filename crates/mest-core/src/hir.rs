use std::fmt;

use chumsky::span::SimpleSpan;
use derive_more::From;
use itertools::Itertools;

use crate::ast::{BinOp, Ident, Literal, UnaryOp};

#[derive(Debug, Clone, Copy, From, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeVar(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    Var(TypeVar),
    Arrow(Box<Type>, Box<Type>),
    Int,
    Float,
    Bool,
    Tuple(Vec<Type>),
    Error,
    Never,
}

#[derive(Debug, Clone)]
pub struct TExpr<'bump> {
    pub kind: &'bump TExprKind<'bump>,
    pub span: SimpleSpan,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub enum TExprKind<'bump> {
    Literal(Literal),
    Ident(Ident),
    If {
        cond: TExpr<'bump>,
        then_expr: TExpr<'bump>,
        else_expr: TExpr<'bump>,
    },
    BinOp {
        op: BinOp,
        lhs: TExpr<'bump>,
        rhs: TExpr<'bump>,
    },
    UnaryOp {
        op: UnaryOp,
        rhs: TExpr<'bump>,
    },
    Let {
        name: Ident,
        value: TExpr<'bump>,
        body: TExpr<'bump>,
        rec: bool,
    },
    Match {
        scrutinee: TExpr<'bump>,
        arms: &'bump [(TPat<'bump>, TExpr<'bump>)],
    },
    Lambda {
        param: Ident,
        param_ty: Type,
        body: TExpr<'bump>,
    },
    App {
        func: TExpr<'bump>,
        arg: TExpr<'bump>,
    },
}

#[derive(Debug, Clone)]
pub enum TPat<'bump> {
    Wildcard,
    Var(Ident, Type),
    Lit(Literal),
    Or(&'bump TPat<'bump>, &'bump TPat<'bump>),
}

const GREEK_LETTERS: &[&str] = &[
    "α", "β", "γ", "δ", "ε", "ζ", "η", "θ", "ι", "κ", "λ", "μ", "ν", "ξ", "ο", "π", "ρ", "σ", "τ",
    "υ", "φ", "χ", "ψ", "ω",
];

impl fmt::Display for TypeVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match GREEK_LETTERS.get(self.0 as usize) {
            Some(name) => f.write_str(name),
            None => write!(f, "α<{}>", self.0),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Var(type_var) => write!(f, "{}", type_var),
            Type::Arrow(from, to) => write!(f, "({}) -> {}", from, to),
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::Tuple(items) => write!(f, "({})", items.iter().format(",")),
            Type::Error => write!(f, "<error>"),
            Type::Never => write!(f, "Never"),
        }
    }
}
