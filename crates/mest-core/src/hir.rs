use chumsky::span::SimpleSpan;
use derive_more::From;

use crate::ast::{BinOp, Ident, Literal, UnaryOp};

#[derive(Debug, Clone, Copy, From)]
pub struct TypeVar(u64);

#[derive(Debug, Clone)]
pub enum Type {
    Var(TypeVar),
    Arrow(Box<Type>, Box<Type>),
    Int,
    Float,
    Bool,
    Tuple(Vec<Type>),
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
