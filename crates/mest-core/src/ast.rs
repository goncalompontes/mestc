use bumpalo::Bump;
use chumsky::span::SimpleSpan;
use lasso::{Rodeo, Spur};
use std::{cell::RefCell, fmt, hash::Hash, ops::Deref, rc::Rc};

use crate::thunk::Thunk;

#[derive(Debug, Clone)]
pub struct VariantDef<'bump> {
    pub name: Ident,
    pub arg: Option<TypeExpr<'bump>>,
}

#[derive(Debug, Clone)]
pub struct TypeDef<'bump> {
    pub name: Ident,
    pub params: &'bump [Ident],
    pub variants: &'bump [VariantDef<'bump>],
}

#[derive(Debug, Clone, Copy)]
pub struct TypeExpr<'bump> {
    pub kind: &'bump TypeExprKind<'bump>,
    pub span: SimpleSpan,
}

impl<'bump> Deref for TypeExpr<'bump> {
    type Target = TypeExprKind<'bump>;
    fn deref(&self) -> &Self::Target {
        self.kind
    }
}

#[derive(Debug, Clone)]
pub enum TypeExprKind<'bump> {
    /// Built-in type: Int, Float, Bool
    Builtin(&'static str),
    /// Type variable (lowercase ident)
    Var(Ident),
    /// Named type constructor (uppercase ident, including user-defined)
    Named(Ident),
    /// Tuple type (T1, T2, ...)
    Tuple(&'bump [TypeExpr<'bump>]),
    /// Function arrow A -> B
    Arrow {
        from: &'bump TypeExpr<'bump>,
        to: &'bump TypeExpr<'bump>,
    },
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Copy, Clone, Debug)]
pub struct Ident {
    pub name: Spur,
    pub span: SimpleSpan,
}

impl Hash for Ident {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Hash::hash(&self.name, state);
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Ident {}

impl PartialOrd for Ident {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ident {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Eq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy)]
pub struct Pat<'bump> {
    pub kind: &'bump PatKind<'bump>,
    pub span: SimpleSpan,
}

impl<'bump> Deref for Pat<'bump> {
    type Target = PatKind<'bump>;

    fn deref(&self) -> &Self::Target {
        self.kind
    }
}

#[derive(Debug, Clone)]
pub enum PatKind<'bump> {
    Wildcard,
    Var(Ident),
    Lit(Literal),
    Or(&'bump Pat<'bump>, &'bump Pat<'bump>),
    Tuple(&'bump [Pat<'bump>]),
    Constructor {
        name: Ident,
        arg: Option<&'bump Pat<'bump>>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Expr<'bump> {
    pub kind: &'bump ExprKind<'bump>,
    pub span: SimpleSpan,
}

impl<'bump> Deref for Expr<'bump> {
    type Target = ExprKind<'bump>;

    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BindPat<'bump> {
    pub kind: &'bump BindPatKind<'bump>,
    pub span: SimpleSpan,
}

#[derive(Debug, Clone)]
pub enum BindPatKind<'bump> {
    Var(Ident),
    Tuple(&'bump [BindPat<'bump>]),
}

#[derive(Debug, Clone)]
pub struct Binding<'bump> {
    pub rec: bool,
    pub pat: BindPat<'bump>,
    pub value: Expr<'bump>,
}

impl<'bump> BindPat<'bump> {
    pub fn var(bump: &'bump Bump, ident: Ident, span: SimpleSpan) -> Self {
        Self {
            kind: bump.alloc(BindPatKind::Var(ident)),
            span,
        }
    }

    pub fn tuple(
        bump: &'bump Bump,
        pats: impl ExactSizeIterator<Item = BindPat<'bump>>,
        span: SimpleSpan,
    ) -> Self {
        Self {
            kind: bump.alloc(BindPatKind::Tuple(bump.alloc_slice_fill_iter(pats))),
            span,
        }
    }
}

impl<'bump> Binding<'bump> {
    pub fn new(_bump: &'bump Bump, rec: bool, pat: BindPat<'bump>, value: Expr<'bump>) -> Self {
        Self { rec, pat, value }
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind<'bump> {
    Literal(Literal),
    Var(Ident),
    If {
        cond: Expr<'bump>,
        then_expr: Expr<'bump>,
        else_expr: Expr<'bump>,
    },
    BinOp {
        op: BinOp,
        lhs: Expr<'bump>,
        rhs: Expr<'bump>,
    },
    UnaryOp {
        op: UnaryOp,
        rhs: Expr<'bump>,
    },
    Let {
        bindings: &'bump [Binding<'bump>],
        body: Expr<'bump>,
    },
    Match {
        scrutinee: Expr<'bump>,
        arms: &'bump [(Pat<'bump>, Expr<'bump>)],
    },
    Abs {
        param: Pat<'bump>,
        body: Expr<'bump>,
    },
    App {
        func: Expr<'bump>,
        arg: Expr<'bump>,
    },
    Tuple(&'bump [Expr<'bump>]),
    Type {
        definitions: &'bump [TypeDef<'bump>],
        body: Expr<'bump>,
    },
    Constructor {
        name: Ident,
        arg: Option<Expr<'bump>>,
    },
}

pub type Env<'bump> = im::HashMap<Ident, Thunk<'bump>>;

#[derive(Debug, Clone)]
pub enum Value<'bump> {
    Int(i64),
    Float(f64),
    Bool(bool),
    Closure {
        param: Pat<'bump>,
        body: Expr<'bump>,
        env: Env<'bump>,
    },
    Tuple(Vec<Value<'bump>>),
    Constructor {
        name: Spur,
        arg: Option<Box<Value<'bump>>>,
    },
}

impl Value<'_> {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::Closure { .. } => "closure",
            Value::Tuple(_) => "tuple",
            Value::Constructor { .. } => "constructor",
        }
    }
}

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Closure { .. } => write!(f, "<closure>"),
            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            Value::Constructor { name, arg } => {
                write!(f, "{:?}", name)?;
                if let Some(arg) = arg {
                    write!(f, " {}", arg)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum EvalError {
    UnboundVariable(String),
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },
    DivisionByZero,
    NotAFunction,
    NonExhaustiveMatch,
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::NonExhaustiveMatch => write!(f, "non-exhaustive match"),
            EvalError::UnboundVariable(name) => write!(f, "unbound variable `{name}`"),
            EvalError::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
            EvalError::DivisionByZero => write!(f, "division by zero"),
            EvalError::NotAFunction => write!(f, "applied a non-function value"),
        }
    }
}

impl<'bump> ExprKind<'bump> {
    fn node(expr: &'bump ExprKind<'bump>, span: SimpleSpan) -> Expr<'bump> {
        Expr { kind: expr, span }
    }

    pub fn literal(bump: &'bump Bump, span: SimpleSpan, lit: Literal) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::Literal(lit)), span)
    }

    pub fn ident(bump: &'bump Bump, span: SimpleSpan, name: Ident) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::Var(name)), span)
    }

    pub fn if_expr(
        bump: &'bump Bump,
        span: SimpleSpan,
        cond: Expr<'bump>,
        then_expr: Expr<'bump>,
        else_expr: Expr<'bump>,
    ) -> Expr<'bump> {
        Self::node(
            bump.alloc(ExprKind::If {
                cond,
                then_expr,
                else_expr,
            }),
            span,
        )
    }

    pub fn binop(
        bump: &'bump Bump,
        span: SimpleSpan,
        op: BinOp,
        lhs: Expr<'bump>,
        rhs: Expr<'bump>,
    ) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::BinOp { op, lhs, rhs }), span)
    }

    pub fn unaryop(
        bump: &'bump Bump,
        span: SimpleSpan,
        op: UnaryOp,
        rhs: Expr<'bump>,
    ) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::UnaryOp { op, rhs }), span)
    }

    pub fn let_expr(
        bump: &'bump Bump,
        span: SimpleSpan,
        bindings: &'bump [Binding<'bump>],
        body: Expr<'bump>,
    ) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::Let { bindings, body }), span)
    }

    pub fn match_expr(
        bump: &'bump Bump,
        span: SimpleSpan,
        scrutinee: Expr<'bump>,
        arms: &'bump [(Pat<'bump>, Expr<'bump>)],
    ) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::Match { scrutinee, arms }), span)
    }

    pub fn lambda(
        bump: &'bump Bump,
        span: SimpleSpan,
        param: Pat<'bump>,
        body: Expr<'bump>,
    ) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::Abs { param, body }), span)
    }

    pub fn app(
        bump: &'bump Bump,
        span: SimpleSpan,
        func: Expr<'bump>,
        arg: Expr<'bump>,
    ) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::App { func, arg }), span)
    }

    pub fn tuple_expr(
        bump: &'bump Bump,
        span: SimpleSpan,
        items: &'bump [Expr<'bump>],
    ) -> Expr<'bump> {
        Self::node(bump.alloc(ExprKind::Tuple(items)), span)
    }
}

impl<'bump> ExprKind<'bump> {
    fn force(thunk: &Thunk<'bump>, rodeo: &Rodeo) -> Result<Value<'bump>, EvalError> {
        thunk.force(rodeo)
    }

    pub fn thunk(expr: &'bump ExprKind<'bump>, env: &Env<'bump>) -> Thunk<'bump> {
        Thunk::new(expr, env.clone())
    }

    pub fn eval_lazy(
        &'bump self,
        env: &Env<'bump>,
        rodeo: &Rodeo,
    ) -> Result<Value<'bump>, EvalError> {
        match self {
            ExprKind::Literal(Literal::Bool(b)) => Ok(Value::Bool(*b)),
            ExprKind::Literal(Literal::Int(v)) => Ok(Value::Int(*v)),
            ExprKind::Literal(Literal::Float(v)) => Ok(Value::Float(*v)),

            ExprKind::Var(ident) => {
                let thunk = env.get(ident).ok_or_else(|| {
                    EvalError::UnboundVariable(rodeo.resolve(&ident.name).to_owned())
                })?;
                Self::force(thunk, rodeo)
            }

            ExprKind::UnaryOp { op, rhs } => {
                let rhs = rhs.kind.eval_lazy(env, rodeo)?;
                match (op, rhs) {
                    (UnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
                    (UnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
                    (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    (UnaryOp::Neg, v) => Err(EvalError::TypeMismatch {
                        expected: "number",
                        got: v.type_name(),
                    }),
                    (UnaryOp::Not, v) => Err(EvalError::TypeMismatch {
                        expected: "bool",
                        got: v.type_name(),
                    }),
                }
            }

            ExprKind::BinOp { op, lhs, rhs } => {
                let lhs = lhs.kind.eval_lazy(env, rodeo)?;
                let rhs = rhs.kind.eval_lazy(env, rodeo)?;
                Self::eval_binop(op, lhs, rhs)
            }

            /*
            ExprKind::Let {
                bindings,
                body,
                rec: true,
            } => {
                let rec_env = Rc::new(RefCell::new(env.clone()));
                let mut thunks: Vec<(Ident, Thunk<'bump>)> = Vec::new();
                for (name, value) in bindings.iter() {
                    let thunk = Thunk::new_shared(value.kind, Rc::clone(&rec_env));
                    rec_env.borrow_mut().insert(*name, thunk.clone());
                    thunks.push((*name, thunk));
                }
                let mut body_env = env.clone();
                for (name, thunk) in thunks {
                    body_env.insert(name, thunk);
                }
                body.kind.eval_lazy(&body_env, rodeo)
            }

            ExprKind::Let {
                bindings,
                body,
                rec: false,
            } => {
                let mut env = env.clone();
                for (name, value) in bindings.iter() {
                    env.insert(*name, Self::thunk(value.kind, &env));
                }
                body.kind.eval_lazy(&env, rodeo)
            }
            */
            ExprKind::Let { bindings, body } => {
                if bindings.iter().any(|b| b.rec) {
                    let rec_env = Rc::new(RefCell::new(env.clone()));
                    for binding in bindings.iter() {
                        if let BindPatKind::Var(ident) = binding.pat.kind {
                            let thunk = Thunk::new_shared(binding.value.kind, Rc::clone(&rec_env));
                            rec_env.borrow_mut().insert(*ident, thunk);
                        }
                    }
                    let mut body_env = env.clone();
                    for binding in bindings.iter() {
                        match binding.pat.kind {
                            BindPatKind::Var(ident) => {
                                let thunk = rec_env.borrow().get(ident).unwrap().clone();
                                body_env.insert(*ident, thunk);
                            }
                            BindPatKind::Tuple(_) => {
                                let thunk =
                                    Thunk::new_shared(binding.value.kind, Rc::clone(&rec_env));
                                Self::bind_pat_to_env(&binding.pat, &thunk, &mut body_env, rodeo)?;
                            }
                        }
                    }
                    body.kind.eval_lazy(&body_env, rodeo)
                } else {
                    let mut body_env = env.clone();
                    for binding in bindings.iter() {
                        let thunk = Self::thunk(binding.value.kind, &body_env);
                        Self::bind_pat_to_env(&binding.pat, &thunk, &mut body_env, rodeo)?;
                    }
                    body.kind.eval_lazy(&body_env, rodeo)
                }
            }

            ExprKind::Match { scrutinee, arms } => {
                let scrutinee_thunk = Thunk::new(scrutinee.kind, env.clone());
                for (pat, body) in arms.iter() {
                    let mut arm_env = env.clone();
                    if Self::match_pat(pat, &scrutinee_thunk, &mut arm_env, rodeo)? {
                        return body.kind.eval_lazy(&arm_env, rodeo);
                    }
                }
                Err(EvalError::NonExhaustiveMatch)
            }

            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => match cond.kind.eval_lazy(env, rodeo)? {
                Value::Bool(true) => then_expr.kind.eval_lazy(env, rodeo),
                Value::Bool(false) => else_expr.kind.eval_lazy(env, rodeo),
                v => Err(EvalError::TypeMismatch {
                    expected: "bool",
                    got: v.type_name(),
                }),
            },

            ExprKind::Abs { param, body } => Ok(Value::Closure {
                param: *param,
                body: *body,
                env: env.clone(),
            }),

            ExprKind::App { func, arg } => {
                let func = func.kind.eval_lazy(env, rodeo)?;
                match func {
                    Value::Closure {
                        param,
                        body,
                        env: mut closure_env,
                    } => {
                        let arg_thunk = Thunk::new(arg.kind, env.clone());
                        Self::match_pat(&param, &arg_thunk, &mut closure_env, rodeo)?;
                        body.kind.eval_lazy(&closure_env, rodeo)
                    }
                    _ => Err(EvalError::NotAFunction),
                }
            }

            ExprKind::Tuple(items) => {
                let mut values = Vec::new();
                for item in items.iter() {
                    values.push(item.kind.eval_lazy(env, rodeo)?);
                }
                Ok(Value::Tuple(values))
            }

            ExprKind::Constructor { name, arg } => {
                let arg = match arg {
                    Some(arg) => Some(Box::new(arg.kind.eval_lazy(env, rodeo)?)),
                    None => None,
                };
                Ok(Value::Constructor {
                    name: name.name,
                    arg,
                })
            }

            ExprKind::Type { definitions: _, body } => {
                // Type definitions are handled during typechecking.
                // At runtime, constructors are just data; no special setup needed.
                body.kind.eval_lazy(env, rodeo)
            }
        }
    }

    fn match_pat(
        pat: &Pat<'bump>,
        thunk: &Thunk<'bump>,
        env: &mut Env<'bump>,
        rodeo: &Rodeo,
    ) -> Result<bool, EvalError> {
        match &**pat {
            PatKind::Wildcard => Ok(true),
            PatKind::Var(name) => {
                env.insert(*name, thunk.clone());
                Ok(true)
            }
            PatKind::Lit(lit) => {
                let val = thunk.force(rodeo)?;
                Ok(match (lit, &val) {
                    (Literal::Int(a), Value::Int(b)) => a == b,
                    (Literal::Float(a), Value::Float(b)) => a == b,
                    (Literal::Bool(a), Value::Bool(b)) => a == b,
                    _ => false,
                })
            }
            PatKind::Or(a, b) => {
                let mut env_a = env.clone();
                if Self::match_pat(a, thunk, &mut env_a, rodeo)? {
                    *env = env_a;
                    Ok(true)
                } else {
                    Self::match_pat(b, thunk, env, rodeo)
                }
            }
            PatKind::Tuple(pats) => {
                let val = thunk.force(rodeo)?;
                match val {
                    Value::Tuple(items) => {
                        if pats.len() != items.len() {
                            return Ok(false);
                        }
                        let mut temp_env = env.clone();
                        for (pat, item_val) in pats.iter().zip(items.iter()) {
                            let item_thunk = Thunk::from_value(item_val.clone(), temp_env.clone());
                            if !Self::match_pat(pat, &item_thunk, &mut temp_env, rodeo)? {
                                return Ok(false);
                            }
                        }
                        *env = temp_env;
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
            PatKind::Constructor { name, arg } => {
                let val = thunk.force(rodeo)?;
                match val {
                    Value::Constructor {
                        name: ref val_name,
                        arg: ref val_arg,
                    } if *val_name == name.name => match (arg, val_arg) {
                        (None, _) => Ok(true),
                        (Some(pat), Some(data)) => {
                            let data_val = (**data).clone();
                            let data_thunk = Thunk::from_value(data_val, env.clone());
                            Self::match_pat(pat, &data_thunk, env, rodeo)
                        }
                        _ => Ok(false),
                    },
                    _ => Ok(false),
                }
            }
        }
    }

    fn bind_pat_to_env(
        pat: &BindPat<'bump>,
        thunk: &Thunk<'bump>,
        env: &mut Env<'bump>,
        rodeo: &Rodeo,
    ) -> Result<(), EvalError> {
        match pat.kind {
            BindPatKind::Var(ident) => {
                env.insert(*ident, thunk.clone());
                Ok(())
            }
            BindPatKind::Tuple(pats) => {
                let val = thunk.force(rodeo)?;
                match val {
                    Value::Tuple(ref items) => {
                        if pats.len() != items.len() {
                            return Err(EvalError::TypeMismatch {
                                expected: "tuple",
                                got: val.type_name(),
                            });
                        }
                        for (sub_pat, item_val) in pats.iter().zip(items.iter()) {
                            let item_env = thunk.env_cell().borrow().clone();
                            let item_thunk = Thunk::from_value(item_val.clone(), item_env);
                            Self::bind_pat_to_env(sub_pat, &item_thunk, env, rodeo)?;
                        }
                        Ok(())
                    }
                    _ => Err(EvalError::TypeMismatch {
                        expected: "tuple",
                        got: val.type_name(),
                    }),
                }
            }
        }
    }

    fn eval_binop(
        op: &BinOp,
        lhs: Value<'bump>,
        rhs: Value<'bump>,
    ) -> Result<Value<'bump>, EvalError> {
        match (op, &lhs, &rhs) {
            (BinOp::And, Value::Bool(l), Value::Bool(r)) => return Ok(Value::Bool(*l && *r)),
            (BinOp::Or, Value::Bool(l), Value::Bool(r)) => return Ok(Value::Bool(*l || *r)),
            (BinOp::And, _, _) | (BinOp::Or, _, _) => {
                return Err(EvalError::TypeMismatch {
                    expected: "bool",
                    got: lhs.type_name(),
                });
            }
            _ => {}
        }

        match op {
            BinOp::Eq => return Ok(Value::Bool(Self::values_equal(&lhs, &rhs)?)),
            BinOp::NotEq => return Ok(Value::Bool(!Self::values_equal(&lhs, &rhs)?)),
            _ => {}
        }

        match (lhs, rhs) {
            (Value::Int(l), Value::Int(r)) => Self::eval_int_binop(op, l, r),
            (Value::Float(l), Value::Float(r)) => Self::eval_float_binop(op, l, r),
            (Value::Int(l), Value::Float(r)) => Self::eval_float_binop(op, l as f64, r),
            (Value::Float(l), Value::Int(r)) => Self::eval_float_binop(op, l, r as f64),
            (lhs, _) => Err(EvalError::TypeMismatch {
                expected: "number",
                got: lhs.type_name(),
            }),
        }
    }

    fn values_equal(lhs: &Value, rhs: &Value) -> Result<bool, EvalError> {
        match (lhs, rhs) {
            (Value::Int(l), Value::Int(r)) => Ok(l == r),
            (Value::Float(l), Value::Float(r)) => Ok(l == r),
            (Value::Bool(l), Value::Bool(r)) => Ok(l == r),
            (Value::Int(l), Value::Float(r)) => Ok((*l as f64) == *r),
            (Value::Float(l), Value::Int(r)) => Ok(*l == (*r as f64)),
            (l, r) => Err(EvalError::TypeMismatch {
                expected: l.type_name(),
                got: r.type_name(),
            }),
        }
    }

    fn eval_int_binop(op: &BinOp, lhs: i64, rhs: i64) -> Result<Value<'bump>, EvalError> {
        match op {
            BinOp::Lt => return Ok(Value::Bool(lhs < rhs)),
            BinOp::Gt => return Ok(Value::Bool(lhs > rhs)),
            BinOp::Le => return Ok(Value::Bool(lhs <= rhs)),
            BinOp::Ge => return Ok(Value::Bool(lhs >= rhs)),
            _ => {}
        }

        Ok(Value::Int(match op {
            BinOp::Add => lhs + rhs,
            BinOp::Sub => lhs - rhs,
            BinOp::Mul => lhs * rhs,
            BinOp::Div => {
                if rhs == 0 {
                    return Err(EvalError::DivisionByZero);
                }
                lhs / rhs
            }
            BinOp::Pow => lhs.pow(rhs as u32),
            _ => unreachable!(),
        }))
    }

    fn eval_float_binop(op: &BinOp, lhs: f64, rhs: f64) -> Result<Value<'bump>, EvalError> {
        match op {
            BinOp::Lt => return Ok(Value::Bool(lhs < rhs)),
            BinOp::Gt => return Ok(Value::Bool(lhs > rhs)),
            BinOp::Le => return Ok(Value::Bool(lhs <= rhs)),
            BinOp::Ge => return Ok(Value::Bool(lhs >= rhs)),
            _ => {}
        }

        Ok(Value::Float(match op {
            BinOp::Add => lhs + rhs,
            BinOp::Sub => lhs - rhs,
            BinOp::Mul => lhs * rhs,
            BinOp::Div => {
                if rhs == 0.0 {
                    return Err(EvalError::DivisionByZero);
                }
                lhs / rhs
            }
            BinOp::Pow => lhs.powf(rhs),
            _ => unreachable!(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;
    use chumsky::span::Span;
    use lasso::Rodeo;

    fn make_ident(rodeo: &mut Rodeo, name: &str) -> Ident {
        let spur = rodeo.get_or_intern(name);
        Ident {
            name: spur,
            span: SimpleSpan::new((), 0..0),
        }
    }

    fn lit_int(bump: &Bump, n: i64) -> Expr<'_> {
        ExprKind::literal(bump, SimpleSpan::new((), 0..0), Literal::Int(n))
    }

    fn span() -> SimpleSpan {
        SimpleSpan::new((), 0..0)
    }

    fn eval<'bump>(expr: &Expr<'bump>) -> Result<Value<'bump>, EvalError> {
        let rodeo = Rodeo::default();
        let env = Env::default();
        expr.kind.eval_lazy(&env, &rodeo)
    }

    // ── Value::Constructor display and type_name ──────────────────────

    #[test]
    fn test_constructor_display_no_arg() {
        let mut rodeo = Rodeo::default();
        let name = rodeo.get_or_intern("None");
        let val = Value::Constructor { name, arg: None };
        let s = format!("{val}");
        assert_eq!(s, format!("{:?}", name));
    }

    #[test]
    fn test_constructor_display_with_arg() {
        let mut rodeo = Rodeo::default();
        let name = rodeo.get_or_intern("Some");
        let val = Value::Constructor {
            name,
            arg: Some(Box::new(Value::Int(42))),
        };
        let s = format!("{val}");
        assert_eq!(s, format!("{:?} 42", name));
    }

    #[test]
    fn test_constructor_type_name() {
        let mut rodeo = Rodeo::default();
        let name = rodeo.get_or_intern("None");
        let val = Value::Constructor { name, arg: None };
        assert_eq!(val.type_name(), "constructor");
    }

    // ── Constructor expression evaluation ────────────────────────────

    #[test]
    fn test_constructor_expr_no_arg() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let ident = make_ident(&mut rodeo, "None");
        let expr = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: ident,
                arg: None,
            }),
            span(),
        );
        let val = eval(&expr).unwrap();
        assert_eq!(val.type_name(), "constructor");
        match val {
            Value::Constructor { name, arg: None } => {
                assert_eq!(name, ident.name);
            }
            _ => panic!("expected Constructor"),
        }
    }

    #[test]
    fn test_constructor_expr_with_arg() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let ident = make_ident(&mut rodeo, "Some");
        let arg = Box::new(ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: ident,
                arg: None,
            }),
            span(),
        ));
        let outer_ident = make_ident(&mut rodeo, "Outer");
        let expr = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: outer_ident,
                arg: Some(*arg),
            }),
            span(),
        );
        let val = eval(&expr).unwrap();
        match val {
            Value::Constructor {
                name,
                arg: Some(inner),
            } => {
                assert_eq!(name, outer_ident.name);
                match *inner {
                    Value::Constructor { arg: None, .. } => {}
                    _ => panic!("expected nested Constructor"),
                }
            }
            _ => panic!("expected outermost Constructor"),
        }
    }

    // ── Pattern matching with constructors ───────────────────────────

    #[test]
    fn test_match_constructor_no_arg() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let none_ident = make_ident(&mut rodeo, "None");

        let scrutinee = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: none_ident,
                arg: None,
            }),
            span(),
        );

        let pat = Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: none_ident,
                arg: None,
            }),
            span: span(),
        };
        let body = lit_int(&bump, 1);
        let fallback_pat = Pat {
            kind: bump.alloc(PatKind::Wildcard),
            span: span(),
        };
        let fallback_body = lit_int(&bump, 0);
        let arms: &[(Pat, Expr)] = bump.alloc_slice_fill_iter([(pat, body), (fallback_pat, fallback_body)]);

        let match_expr = ExprKind::node(
            bump.alloc(ExprKind::Match { scrutinee, arms }),
            span(),
        );

        let val = eval(&match_expr).unwrap();
        assert!(matches!(val, Value::Int(1)));
    }

    #[test]
    fn test_match_constructor_with_arg() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let some_ident = make_ident(&mut rodeo, "Some");
        let x_ident = make_ident(&mut rodeo, "x");

        let scrutinee = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: some_ident,
                arg: Some(lit_int(&bump, 42)),
            }),
            span(),
        );

        let var_pat = Pat {
            kind: bump.alloc(PatKind::Var(x_ident)),
            span: span(),
        };
        let pat = Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: some_ident,
                arg: Some(&*bump.alloc(var_pat)),
            }),
            span: span(),
        };
        let body = ExprKind::node(
            bump.alloc(ExprKind::Var(x_ident)),
            span(),
        );
        let fallback_pat = Pat {
            kind: bump.alloc(PatKind::Wildcard),
            span: span(),
        };
        let fallback_body = lit_int(&bump, 0);
        let arms: &[(Pat, Expr)] = bump.alloc_slice_fill_iter([(pat, body), (fallback_pat, fallback_body)]);

        let match_expr = ExprKind::node(
            bump.alloc(ExprKind::Match { scrutinee, arms }),
            span(),
        );

        let val = eval(&match_expr).unwrap();
        assert!(matches!(val, Value::Int(42)));
    }

    #[test]
    fn test_match_constructor_wrong_name() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let some_ident = make_ident(&mut rodeo, "Some");
        let none_ident = make_ident(&mut rodeo, "None");

        let scrutinee = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: some_ident,
                arg: None,
            }),
            span(),
        );

        // Match against None — should NOT match Some
        let pat = Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: none_ident,
                arg: None,
            }),
            span: span(),
        };
        let body = lit_int(&bump, 1);
        let fallback_pat = Pat {
            kind: bump.alloc(PatKind::Wildcard),
            span: span(),
        };
        let fallback_body = lit_int(&bump, 0);
        let arms: &[(Pat, Expr)] = bump.alloc_slice_fill_iter([(pat, body), (fallback_pat, fallback_body)]);

        let match_expr = ExprKind::node(
            bump.alloc(ExprKind::Match { scrutinee, arms }),
            span(),
        );

        let val = eval(&match_expr).unwrap();
        assert!(matches!(val, Value::Int(0)));
    }

    #[test]
    fn test_match_constructor_wildcard_fallback() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let some_ident = make_ident(&mut rodeo, "Some");
        let none_ident = make_ident(&mut rodeo, "None");

        let scrutinee = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: some_ident,
                arg: Some(lit_int(&bump, 10)),
            }),
            span(),
        );

        let pat = Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: none_ident,
                arg: None,
            }),
            span: span(),
        };
        let body = lit_int(&bump, 1);
        let wild_pat = Pat {
            kind: bump.alloc(PatKind::Wildcard),
            span: span(),
        };
        let wild_body = lit_int(&bump, 99);
        let arms: &[(Pat, Expr)] = bump.alloc_slice_fill_iter([(pat, body), (wild_pat, wild_body)]);

        let match_expr = ExprKind::node(
            bump.alloc(ExprKind::Match { scrutinee, arms }),
            span(),
        );

        let val = eval(&match_expr).unwrap();
        assert!(matches!(val, Value::Int(99)));
    }

    #[test]
    fn test_match_constructor_arg_wrong_name() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let some_ident = make_ident(&mut rodeo, "Some");
        let none_ident = make_ident(&mut rodeo, "None");

        let scrutinee = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: some_ident,
                arg: None,
            }),
            span(),
        );

        // Match against None (with wildcard arg pattern)
        let wild_pat = Pat {
            kind: bump.alloc(PatKind::Wildcard),
            span: span(),
        };
        let pat = Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: none_ident,
                arg: Some(&*bump.alloc(wild_pat)),
            }),
            span: span(),
        };
        let body = lit_int(&bump, 1);
        let fallback_pat = Pat {
            kind: bump.alloc(PatKind::Wildcard),
            span: span(),
        };
        let fallback_body = lit_int(&bump, 0);
        let arms: &[(Pat, Expr)] = bump.alloc_slice_fill_iter([(pat, body), (fallback_pat, fallback_body)]);

        let match_expr = ExprKind::node(
            bump.alloc(ExprKind::Match { scrutinee, arms }),
            span(),
        );

        let val = eval(&match_expr).unwrap();
        assert!(matches!(val, Value::Int(0)));
    }

    #[test]
    fn test_match_constructor_or_pattern() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let none_ident = make_ident(&mut rodeo, "None");
        let some_ident = make_ident(&mut rodeo, "Some");

        let scrutinee = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: none_ident,
                arg: None,
            }),
            span(),
        );

        // pattern: (Some x | None) => 1, _ => 0
        let some_var = Pat {
            kind: bump.alloc(PatKind::Var(make_ident(&mut rodeo, "x"))),
            span: span(),
        };
        let some_pat = Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: some_ident,
                arg: Some(&*bump.alloc(some_var)),
            }),
            span: span(),
        };
        let none_pat = Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: none_ident,
                arg: None,
            }),
            span: span(),
        };
        let or_pat = Pat {
            kind: bump.alloc(PatKind::Or(
                bump.alloc(some_pat),
                bump.alloc(none_pat),
            )),
            span: span(),
        };
        let body = lit_int(&bump, 1);
        let fallback_pat = Pat {
            kind: bump.alloc(PatKind::Wildcard),
            span: span(),
        };
        let fallback_body = lit_int(&bump, 0);
        let arms: &[(Pat, Expr)] = bump.alloc_slice_fill_iter([(or_pat, body), (fallback_pat, fallback_body)]);

        let match_expr = ExprKind::node(
            bump.alloc(ExprKind::Match { scrutinee, arms }),
            span(),
        );

        let val = eval(&match_expr).unwrap();
        assert!(matches!(val, Value::Int(1)));
    }

    #[test]
    fn test_match_constructor_non_exhaustive() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let some_ident = make_ident(&mut rodeo, "Some");

        let scrutinee = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: some_ident,
                arg: Some(lit_int(&bump, 1)),
            }),
            span(),
        );

        // Only matching None, but scrutinee is Some — non-exhaustive
        let pat = Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: make_ident(&mut rodeo, "None"),
                arg: None,
            }),
            span: span(),
        };
        let body = lit_int(&bump, 1);
        let arms: &[(Pat, Expr)] = bump.alloc_slice_fill_iter([(pat, body)]);

        let match_expr = ExprKind::node(
            bump.alloc(ExprKind::Match { scrutinee, arms }),
            span(),
        );

        let result = eval(&match_expr);
        assert!(matches!(result, Err(EvalError::NonExhaustiveMatch)));
    }

    // ── Type expression evaluation ───────────────────────────────────

    #[test]
    fn test_type_expr_evaluates_body() {
        let bump = Bump::new();
        let mut rodeo = Rodeo::default();
        let none_ident = make_ident(&mut rodeo, "None");
        let option_ident = make_ident(&mut rodeo, "Option");

        let body = ExprKind::node(
            bump.alloc(ExprKind::Constructor {
                name: none_ident,
                arg: None,
            }),
            span(),
        );

        let type_def = TypeDef {
            name: option_ident,
            params: &[],
            variants: &[],
        };
        let type_defs: &[TypeDef] = bump.alloc_slice_fill_iter([type_def]);

        let type_expr = ExprKind::node(
            bump.alloc(ExprKind::Type {
                definitions: type_defs,
                body,
            }),
            span(),
        );

        let val = eval(&type_expr).unwrap();
        match val {
            Value::Constructor { arg: None, .. } => {}
            _ => panic!("expected Constructor from body evaluation"),
        }
    }
}
