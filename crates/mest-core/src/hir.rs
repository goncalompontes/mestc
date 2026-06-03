use std::fmt;

use chumsky::span::SimpleSpan;
use derive_more::From;
use itertools::Itertools;
use lasso::Rodeo;

use crate::ast::{BinOp, Ident, Literal, UnaryOp};

#[derive(Debug, Clone, Copy, From, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeVar(pub u64);

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecordField {
    pub name: Spur,
    pub ty: Type,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    Var(TypeVar),
    Arrow(Box<Type>, Box<Type>),
    Int,
    Float,
    Bool,
    Tuple(Vec<Type>),
    App(Spur, Vec<Type>),
    Record(Vec<RecordField>),
    Unit,
    Error,
    Never,
}

use lasso::Spur;

impl Type {
    pub fn app(name: Spur, args: Vec<Type>) -> Self {
        Type::App(name, args)
    }
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
    "τ", "υ", "ν", "ο", "α", "β", "γ", "δ", "ζ", "η", "θ", "ι", "κ", "ξ", "ρ", "χ", "ψ",
    "ε", "λ", "μ", "π", "σ", "φ", "ω",
];

impl fmt::Display for TypeVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match GREEK_LETTERS.get(self.0 as usize) {
            Some(name) => f.write_str(name),
            None => write!(f, "α<{}>", self.0),
        }
    }
}

fn write_record_fields(f: &mut fmt::Formatter<'_>, fields: &[RecordField]) -> fmt::Result {
    write!(f, "{{ ")?;
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{:?}: {}", field.name, field.ty)?;
    }
    write!(f, " }}")
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
            Type::App(name, args) => {
                if args.is_empty() {
                    write!(f, "{:?}", name)
                } else {
                    write!(f, "{:?} {}", name, args.iter().format(" "))
                }
            }
            Type::Record(fields) => write_record_fields(f, fields),
            Type::Unit => write!(f, "()"),
            Type::Error => write!(f, "<error>"),
            Type::Never => write!(f, "Never"),
        }
    }
}

pub fn type_to_string(ty: &Type, rodeo: &Rodeo) -> String {
    match ty {
        Type::App(name, args) => {
            let name = rodeo.resolve(name);
            if args.is_empty() {
                name.to_string()
            } else {
                let args: Vec<_> = args.iter().map(|a| type_to_string(a, rodeo)).collect();
                format!("{} {}", name, args.join(" "))
            }
        }
        Type::Arrow(from, to) => format!("({}) -> {}", type_to_string(from, rodeo), type_to_string(to, rodeo)),
        Type::Tuple(items) => {
            let items: Vec<_> = items.iter().map(|a| type_to_string(a, rodeo)).collect();
            format!("({})", items.join(", "))
        }
        Type::Record(fields) => {
            let fields: Vec<_> = fields.iter().map(|f| format!("{}: {}", rodeo.resolve(&f.name), type_to_string(&f.ty, rodeo))).collect();
            format!("{{ {} }}", fields.join(", "))
        }
        other => other.to_string(),
    }
}
