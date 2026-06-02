use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use chumsky::span::SimpleSpan;
use im::HashSet;
use itertools::Itertools;
use lasso::Rodeo;
use thiserror::Error;

use crate::{
    ast::{BinOp, Expr, ExprKind, Ident, Literal, Pat, PatKind, UnaryOp},
    hir::{Type, TypeVar},
};

// Type schemes for polymorphic types
#[derive(Debug, Clone, PartialEq)]
pub struct Scheme {
    pub type_vars: Vec<TyVar>,
    pub ty: Type,
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.type_vars.is_empty() {
            write!(f, "{}", self.ty)
        } else {
            write!(f, "forall {}. {}", self.type_vars.iter().join(" "), self.ty)
        }
    }
}

#[derive(Error, Debug)]
pub enum InferenceError {
    #[error("Variable '{name}' not found in environment")]
    UnboundVariable { span: SimpleSpan, name: String },

    #[error("Type mismatch")]
    UnificationFailure {
        span: SimpleSpan,
        expected: Type,
        actual: Type,
    },

    #[error("Occurs check failed: infinite type")]
    OccursCheck {
        span: SimpleSpan,
        var: TypeVar,
        ty: Type,
    },

    #[error("Tuple length mismatch")]
    TupleLengthMismatch {
        span: SimpleSpan,
        left_len: usize,
        right_len: usize,
    },

    #[error("Match expression has no arms")]
    NoMatchArms { span: SimpleSpan },
}

impl InferenceError {
    fn span(&self) -> SimpleSpan {
        match self {
            Self::UnboundVariable { span, .. } => *span,
            Self::UnificationFailure { span, .. } => *span,
            Self::OccursCheck { span, .. } => *span,
            Self::TupleLengthMismatch { span, .. } => *span,
            Self::NoMatchArms { span } => *span,
        }
    }

    fn label(&self) -> String {
        match self {
            Self::UnboundVariable { name, .. } => {
                format!("`{name}` is not defined in this scope")
            }
            Self::UnificationFailure {
                expected, actual, ..
            } => {
                format!("expected `{expected}`, found `{actual}`")
            }
            Self::OccursCheck { var, ty, .. } => {
                format!("type variable `{var}` occurs in `{ty}`")
            }
            Self::TupleLengthMismatch {
                left_len,
                right_len,
                ..
            } => {
                format!("expected {left_len} elements, found {right_len}")
            }
            Self::NoMatchArms { .. } => "add at least one arm to this match expression".into(),
        }
    }

    pub fn to_report(&self) -> ariadne::Report<'_, ((), Range<usize>)> {
        let span = self.span().into_range();
        ariadne::Report::build(ariadne::ReportKind::Error, ((), span.clone()))
            .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
            .with_code(4)
            .with_message(self.to_string())
            .with_label(
                ariadne::Label::new(((), span))
                    .with_message(self.label())
                    .with_color(ariadne::Color::Red),
            )
            .finish()
    }
}

pub type TyVar = TypeVar;
pub type TmVar = Ident;
pub type Env = BTreeMap<TmVar, Scheme>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuleName {
    TVar,
    TAbs,
    TApp,
    TLet,
    TIf,
    TBinOp,
    TUnaryOp,
    TMatch,
    TInt,
    TFloat,
    TBool,
    TError,
    UnifyBase,
    UnifyVarSame,
    UnifyVar,
    UnifyArrow,
    UnifyTuple,
    TPatWild,
    TPatVar,
    TPatLit,
    TPatOr,
}

impl std::fmt::Display for RuleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TVar => write!(f, "T-Var"),
            Self::TAbs => write!(f, "T-Abs"),
            Self::TApp => write!(f, "T-App"),
            Self::TLet => write!(f, "T-Let"),
            Self::TIf => write!(f, "T-If"),
            Self::TBinOp => write!(f, "T-BinOp"),
            Self::TUnaryOp => write!(f, "T-UnaryOp"),
            Self::TMatch => write!(f, "T-Match"),
            Self::TInt => write!(f, "T-Int"),
            Self::TFloat => write!(f, "T-Float"),
            Self::TBool => write!(f, "T-Bool"),
            Self::TError => write!(f, "T-Error"),
            Self::UnifyBase => write!(f, "Unify-Base"),
            Self::UnifyVarSame => write!(f, "Unify-Var-Same"),
            Self::UnifyVar => write!(f, "Unify-Var"),
            Self::UnifyArrow => write!(f, "Unify-Arrow"),
            Self::UnifyTuple => write!(f, "Unify-Tuple"),
            Self::TPatWild => write!(f, "T-Pat-Wild"),
            Self::TPatVar => write!(f, "T-Pat-Var"),
            Self::TPatLit => write!(f, "T-Pat-Lit"),
            Self::TPatOr => write!(f, "T-Pat-Or"),
        }
    }
}

pub enum Input<'a> {
    Infer { env: String, expr: &'a Expr<'a> },
    Unify { left: Type, right: Type },
    Pat(String),
}

pub enum Output {
    Type(Type),
    Str(String),
}

impl From<Type> for Output {
    fn from(ty: Type) -> Self {
        Self::Type(ty)
    }
}

impl From<String> for Output {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<&str> for Output {
    fn from(s: &str) -> Self {
        Self::Str(s.to_owned())
    }
}

pub struct InferenceTree<'a> {
    pub rule: RuleName,
    pub input: Input<'a>,
    pub output: Output,
    pub children: Vec<InferenceTree<'a>>,
}

impl<'a> InferenceTree<'a> {
    pub fn new(
        rule: RuleName,
        input: Input<'a>,
        output: impl Into<Output>,
        children: Vec<InferenceTree<'a>>,
    ) -> InferenceTree<'a> {
        InferenceTree {
            rule,
            input,
            output: output.into(),
            children,
        }
    }

    pub fn display_tree(&self, rodeo: &Rodeo) -> String {
        let mut buf = String::new();
        self.write(&mut buf, &[], rodeo);
        buf
    }

    fn write(&self, buf: &mut String, prefix: &[bool], rodeo: &Rodeo) {
        use std::fmt::Write;
        use yansi::Paint;

        for &is_last in prefix.iter().take(prefix.len().saturating_sub(1)) {
            let ch = if is_last { "    " } else { "│   " };
            let _ = write!(buf, "{}", ch.dim());
        }
        if let Some(&last) = prefix.last() {
            let ch = if last { "└── " } else { "├── " };
            let _ = write!(buf, "{}", ch.dim());
        }

        if self.rule == RuleName::TError {
            let _ = write!(buf, "{}", self.rule.to_string().red().bold());
        } else {
            let _ = write!(buf, "{}", self.rule.to_string().cyan().bold());
        }
        let _ = write!(buf, " ");

        match &self.input {
            Input::Infer { env, expr } => {
                let _ = write!(buf, "{}", env.yellow());
                let _ = write!(buf, "{}", " ⊢ ".yellow());
                let _ = write!(buf, "{}", expr.display_inline(rodeo).yellow());
                let _ = write!(buf, "{}", " ⇒".yellow());
            }
            Input::Unify { left, right } => {
                let _ = write!(buf, "{}", format!("{}", left).yellow());
                let _ = write!(buf, "{}", " ~ ".yellow());
                let _ = write!(buf, "{}", format!("{}", right).yellow());
            }
            Input::Pat(s) => {
                let _ = write!(buf, "{}", s.yellow());
            }
        }
        let _ = write!(buf, " ");

        match &self.output {
            Output::Type(ty) => {
                if self.rule == RuleName::TError {
                    let _ = write!(buf, "{}", format!("{}", ty).red());
                } else {
                    let _ = write!(buf, "{}", format!("{}", ty).yellow());
                }
            }
            Output::Str(s) => {
                if self.rule == RuleName::TError {
                    let _ = write!(buf, "{}", s.red());
                } else {
                    let _ = write!(buf, "{}", s.yellow());
                }
            }
        }
        let _ = writeln!(buf);

        for (i, child) in self.children.iter().enumerate() {
            let is_last = i == self.children.len() - 1;
            let mut child_prefix = prefix.to_vec();
            child_prefix.push(is_last);
            child.write(buf, &child_prefix, rodeo);
        }
    }
}

pub struct TypeCheckResult<'a> {
    pub ty: Type,
    pub errors: Vec<InferenceError>,
    pub tree: InferenceTree<'a>,
}

#[derive(Default)]
struct FreeVars {
    types: HashSet<TyVar>,
}

#[derive(Debug, Clone)]
pub struct Subst {
    pub types: HashMap<TyVar, Type>,
}

impl Subst {
    pub fn compose(&self, other: &Self) -> Self {
        let mut types: HashMap<_, _> = other
            .types
            .iter()
            .map(|(k, ty)| (k.clone(), TypeInference::apply_type(self, ty)))
            .collect();
        for (k, v) in &self.types {
            types.entry(k.clone()).or_insert_with(|| v.clone());
        }
        Subst { types }
    }

    fn singleton_type(var: TyVar, ty: Type) -> Self {
        let mut s = Self::empty();
        s.types.insert(var, ty);
        s
    }

    fn empty() -> Subst {
        Subst {
            types: HashMap::new(),
        }
    }
}

pub struct TypeInference<'rodeo> {
    counter: u64,
    rodeo: &'rodeo mut Rodeo,
    errors: Vec<InferenceError>,
}

impl<'rodeo> TypeInference<'rodeo> {
    fn fresh_tyvar(&mut self) -> TyVar {
        let var = self.counter;
        self.counter += 1;
        TypeVar(var)
    }

    fn emit_error(&mut self, err: InferenceError) {
        self.errors.push(err);
    }

    pub fn take_errors(&mut self) -> Vec<InferenceError> {
        std::mem::take(&mut self.errors)
    }

    fn instantiate(&mut self, scheme: &Scheme) -> Type {
        let mut s = Subst::empty();
        for v in &scheme.type_vars {
            let fresh = self.fresh_tyvar();
            s.types.insert(v.clone(), Type::Var(fresh));
        }
        Self::apply_type(&s, &scheme.ty)
    }

    fn generalize(&self, env: &Env, ty: &Type) -> Scheme {
        let mut fv = FreeVars::default();
        ftv_type(ty, &mut fv);
        let env_fv = ftv_env(env);
        Scheme {
            type_vars: fv
                .types
                .difference(env_fv.types)
                .iter()
                .sorted()
                .cloned()
                .collect(),
            ty: ty.clone(),
        }
    }

    fn apply_type(s: &Subst, ty: &Type) -> Type {
        match ty {
            Type::Var(name) => match s.types.get(name) {
                Some(ty) => Self::apply_type(s, ty),
                None => ty.clone(),
            },
            Type::Arrow(from, to) => Type::Arrow(
                Box::new(Self::apply_type(s, from)),
                Box::new(Self::apply_type(s, to)),
            ),
            Type::Tuple(types) => {
                Type::Tuple(types.iter().map(|ty| Self::apply_type(s, ty)).collect())
            }
            Type::Int | Type::Bool | Type::Float => ty.clone(),
            Type::Error | Type::Never => ty.clone(),
        }
    }

    fn apply_scheme(s: &Subst, scheme: &Scheme) -> Scheme {
        let mut filtered = s.clone();
        for v in &scheme.type_vars {
            filtered.types.remove(v);
        }
        Scheme {
            type_vars: scheme.type_vars.clone(),
            ty: Self::apply_type(&filtered, &scheme.ty),
        }
    }

    fn apply_env(s: &Subst, env: &Env) -> Env {
        env.iter()
            .map(|(k, scheme)| (k.clone(), Self::apply_scheme(s, scheme)))
            .collect()
    }

    fn unify<'tree>(&mut self, this: &Type, other: &Type, span: SimpleSpan) -> (Subst, InferenceTree<'tree>) {
        let error = |this: &'_ Type, other: &'_ Type, msg: &str| {
            let tree = InferenceTree::new(
                RuleName::TError,
                Input::Unify {
                    left: this.clone(),
                    right: other.clone(),
                },
                msg,
                vec![],
            );
            (Subst::empty(), tree)
        };

        match (this, other) {
            (Type::Error, _) | (_, Type::Error) | (Type::Never, _) | (_, Type::Never) => {
                let tree = InferenceTree::new(
                    RuleName::UnifyBase,
                    Input::Unify {
                        left: this.clone(),
                        right: other.clone(),
                    },
                    "{}",
                    vec![],
                );
                (Subst::empty(), tree)
            }
            (Type::Int, Type::Int) | (Type::Float, Type::Float) | (Type::Bool, Type::Bool) => {
                let tree = InferenceTree::new(
                    RuleName::UnifyBase,
                    Input::Unify {
                        left: this.clone(),
                        right: other.clone(),
                    },
                    "{}",
                    vec![],
                );
                (Subst::empty(), tree)
            }
            (Type::Var(var), ty) | (ty, Type::Var(var)) => {
                if ty == &Type::Var(var.clone()) {
                    let tree = InferenceTree::new(
                        RuleName::UnifyVarSame,
                        Input::Unify {
                            left: this.clone(),
                            right: other.clone(),
                        },
                        "{}",
                        vec![],
                    );
                    (Subst::empty(), tree)
                } else if self.occurs_check(var, ty) {
                    self.emit_error(InferenceError::OccursCheck {
                        span,
                        var: var.clone(),
                        ty: ty.clone(),
                    });
                    error(this, other, "occurs check failed")
                } else {
                    let subst = Subst::singleton_type(var.clone(), ty.clone());
                    let output = format!("{{{}/{}}}", ty, var);
                    let tree = InferenceTree::new(
                        RuleName::UnifyVar,
                        Input::Unify {
                            left: this.clone(),
                            right: other.clone(),
                        },
                        output,
                        vec![],
                    );
                    (subst, tree)
                }
            }
            (Type::Arrow(from_a, to_a), Type::Arrow(from_b, to_b)) => {
                let (from, from_tree) = self.unify(from_a, from_b, span);
                let (to, to_tree) = self.unify(
                    &Self::apply_type(&from, to_a),
                    &Self::apply_type(&from, to_b),
                    span,
                );
                let final_subst = to.compose(&from);
                let output = self.pretty_subst(&final_subst);
                let tree = InferenceTree::new(
                    RuleName::UnifyArrow,
                    Input::Unify {
                        left: this.clone(),
                        right: other.clone(),
                    },
                    output,
                    vec![from_tree, to_tree],
                );
                (final_subst, tree)
            }
            (Type::Tuple(ts1), Type::Tuple(ts2)) => {
                if ts1.len() != ts2.len() {
                    self.emit_error(InferenceError::TupleLengthMismatch {
                        span,
                        left_len: ts1.len(),
                        right_len: ts2.len(),
                    });
                    return error(this, other, "tuple length mismatch");
                };

                let mut subst = Subst::empty();
                let mut trees = Vec::new();

                for (t1, t2) in ts1.iter().zip(ts2.iter()) {
                    let (s, tree) = self.unify(
                        &Self::apply_type(&subst, t1),
                        &Self::apply_type(&subst, t2),
                        span,
                    );
                    subst = s.compose(&subst);
                    trees.push(tree);
                }

                let output = self.pretty_subst(&subst);
                let tree = InferenceTree::new(
                    RuleName::UnifyTuple,
                    Input::Unify {
                        left: this.clone(),
                        right: other.clone(),
                    },
                    output,
                    trees,
                );
                (subst, tree)
            }
            _ => {
                self.emit_error(InferenceError::UnificationFailure {
                    span,
                    expected: this.clone(),
                    actual: other.clone(),
                });
                error(this, other, "type mismatch")
            }
        }
    }

    pub fn infer<'a>(&mut self, env: &Env, expr: &'a Expr<'a>) -> (Subst, Type, InferenceTree<'a>) {
        match &**expr {
            ExprKind::Literal(literal) => self.infer_lit(env, expr, literal),
            ExprKind::Var(ident) => self.infer_var(env, expr, *ident),
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => self.infer_if(env, expr, cond, then_expr, else_expr),
            ExprKind::BinOp { op, lhs, rhs } => self.infer_binop(env, expr, op, lhs, rhs),
            ExprKind::UnaryOp { op, rhs } => self.infer_unaryop(env, expr, op, rhs),
            ExprKind::Let {
                name,
                value,
                body,
                rec: _,
            } => self.infer_let(env, expr, name, value, body),
            ExprKind::Match { scrutinee, arms } => self.infer_match(env, expr, scrutinee, arms),
            ExprKind::Abs { param, body } => self.infer_abs(env, expr, param, body),
            ExprKind::App { func, arg } => self.infer_app(env, expr, func, arg),
        }
    }

    fn infer_var<'a>(&mut self, env: &Env, expr: &'a Expr<'a>, name: Ident) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);
        match env.get(&name) {
            Some(scheme) => {
                let instantiated = self.instantiate(scheme);
                let tree = InferenceTree::new(
                    RuleName::TVar,
                    Input::Infer {
                        env: env_str,
                        expr,
                    },
                    instantiated.clone(),
                    vec![],
                );
                (Subst::empty(), instantiated, tree)
            }
            None => {
                let err = InferenceError::UnboundVariable {
                    span: expr.span,
                    name: self.rodeo.resolve(&name.0).to_owned(),
                };
                self.emit_error(err);
                let tree = InferenceTree::new(
                    RuleName::TError,
                    Input::Infer {
                        env: env_str,
                        expr,
                    },
                    "unbound variable",
                    vec![],
                );
                (Subst::empty(), Type::Error, tree)
            }
        }
    }

    fn infer_abs<'a>(
        &mut self,
        env: &Env,
        expr: &'a Expr<'a>,
        param: &TmVar,
        body: &'a Expr<'a>,
    ) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);

        let param_ty = Type::Var(self.fresh_tyvar());
        let mut new_env = env.clone();
        let param_scheme = Scheme {
            type_vars: vec![],
            ty: param_ty.clone(),
        };
        new_env.insert(*param, param_scheme);

        let (s1, body_ty, tree1) = self.infer(&new_env, body);
        let result_ty = Type::Arrow(
            Box::new(Self::apply_type(&s1, &param_ty)),
            Box::new(body_ty),
        );

        let tree = InferenceTree::new(
            RuleName::TAbs,
            Input::Infer {
                env: env_str,
                expr,
            },
            result_ty.clone(),
            vec![tree1],
        );
        (s1, result_ty, tree)
    }

    fn infer_app<'a>(
        &mut self,
        env: &Env,
        expr: &'a Expr<'a>,
        func: &'a Expr<'a>,
        arg: &'a Expr<'a>,
    ) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);

        let result_ty = Type::Var(self.fresh_tyvar());

        let (s1, func_ty, tree1) = self.infer(env, func);
        let env_subst = Self::apply_env(&s1, env);
        let (s2, arg_ty, tree2) = self.infer(&env_subst, arg);

        let func_ty_subst = Self::apply_type(&s2, &func_ty);
        let expected_func_ty = Type::Arrow(Box::new(arg_ty), Box::new(result_ty.clone()));

        let (s3, tree3) = self.unify(&func_ty_subst, &expected_func_ty, func.span);

        let final_subst = s3.compose(&s2.compose(&s1));
        let final_ty = Self::apply_type(&s3, &result_ty);

        let tree = InferenceTree::new(
            RuleName::TApp,
            Input::Infer {
                env: env_str,
                expr,
            },
            final_ty.clone(),
            vec![tree1, tree2, tree3],
        );
        (final_subst, final_ty, tree)
    }

    fn infer_let<'a>(
        &mut self,
        env: &Env,
        expr: &'a Expr<'a>,
        var: &TmVar,
        value: &'a Expr<'a>,
        body: &'a Expr<'a>,
    ) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);

        let (s1, value_ty, tree1) = self.infer(env, value);
        let env_subst = Self::apply_env(&s1, env);
        let generalized_ty = self.generalize(&env_subst, &value_ty);

        let mut new_env = env_subst;
        new_env.insert(var.clone(), generalized_ty);

        let (s2, body_ty, tree2) = self.infer(&new_env, body);

        let final_subst = s2.compose(&s1);
        let tree = InferenceTree::new(
            RuleName::TLet,
            Input::Infer {
                env: env_str,
                expr,
            },
            body_ty.clone(),
            vec![tree1, tree2],
        );
        (final_subst, body_ty, tree)
    }

    fn infer_if<'a>(
        &mut self,
        env: &Env,
        expr: &'a Expr<'a>,
        cond: &'a Expr<'a>,
        then_expr: &'a Expr<'a>,
        else_expr: &'a Expr<'a>,
    ) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);

        let (s1, cond_ty, tree1) = self.infer(env, cond);
        let env1 = Self::apply_env(&s1, env);
        let (s2, then_ty, tree2) = self.infer(&env1, then_expr);
        let env2 = Self::apply_env(&s2, &env1);
        let (s3, else_ty, tree3) = self.infer(&env2, else_expr);

        let cond_ty_s = Self::apply_type(&s3, &Self::apply_type(&s2, &cond_ty));
        let (s4, cond_tree) = self.unify(&cond_ty_s, &Type::Bool, cond.span);

        let then_ty_s = Self::apply_type(&s4, &Self::apply_type(&s3, &then_ty));
        let else_ty_s = Self::apply_type(&s4, &else_ty);
        let (s5, eq_tree) = self.unify(&then_ty_s, &else_ty_s, then_expr.span);

        let final_subst = s5.compose(&s4.compose(&s3.compose(&s2.compose(&s1))));
        let final_ty = Self::apply_type(&final_subst, &then_ty);

        let tree = InferenceTree::new(
            RuleName::TIf,
            Input::Infer {
                env: env_str,
                expr,
            },
            final_ty.clone(),
            vec![tree1, tree2, tree3, cond_tree, eq_tree],
        );
        (final_subst, final_ty, tree)
    }

    fn infer_binop<'a>(
        &mut self,
        env: &Env,
        expr: &'a Expr<'a>,
        op: &BinOp,
        lhs: &'a Expr<'a>,
        rhs: &'a Expr<'a>,
    ) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);

        let (expected_lhs_ty, expected_rhs_ty, result_ty) = match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow => {
                (Type::Int, Type::Int, Type::Int)
            }
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                (Type::Int, Type::Int, Type::Bool)
            }
            BinOp::And | BinOp::Or => (Type::Bool, Type::Bool, Type::Bool),
        };

        let (s1, lhs_ty, tree1) = self.infer(env, lhs);
        let env1 = Self::apply_env(&s1, env);
        let (s2, rhs_ty, tree2) = self.infer(&env1, rhs);

        let lhs_ty_s = Self::apply_type(&s2, &lhs_ty);
        let (s3, lhs_tree) = self.unify(&lhs_ty_s, &expected_lhs_ty, lhs.span);

        let rhs_ty_s = Self::apply_type(&s3, &rhs_ty);
        let (s4, rhs_tree) = self.unify(&rhs_ty_s, &expected_rhs_ty, rhs.span);

        let final_subst = s4.compose(&s3.compose(&s2.compose(&s1)));
        let final_ty = Self::apply_type(&final_subst, &result_ty);

        let tree = InferenceTree::new(
            RuleName::TBinOp,
            Input::Infer {
                env: env_str,
                expr,
            },
            final_ty.clone(),
            vec![tree1, tree2, lhs_tree, rhs_tree],
        );
        (final_subst, final_ty, tree)
    }

    fn infer_unaryop<'a>(
        &mut self,
        env: &Env,
        expr: &'a Expr<'a>,
        op: &UnaryOp,
        rhs: &'a Expr<'a>,
    ) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);

        let (expected_arg_ty, result_ty) = match op {
            UnaryOp::Neg => (Type::Int, Type::Int),
            UnaryOp::Not => (Type::Bool, Type::Bool),
        };

        let (s1, rhs_ty, tree1) = self.infer(env, rhs);
        let rhs_ty_s = Self::apply_type(&s1, &rhs_ty);
        let (s2, rhs_tree) = self.unify(&rhs_ty_s, &expected_arg_ty, rhs.span);

        let final_subst = s2.compose(&s1);
        let final_ty = Self::apply_type(&final_subst, &result_ty);

        let tree = InferenceTree::new(
            RuleName::TUnaryOp,
            Input::Infer {
                env: env_str,
                expr,
            },
            final_ty.clone(),
            vec![tree1, rhs_tree],
        );
        (final_subst, final_ty, tree)
    }

    fn infer_pat<'a>(&mut self, pat: &'a Pat<'a>) -> (Type, Vec<(Ident, Scheme)>, InferenceTree<'a>) {
        match &**pat {
            PatKind::Wildcard => {
                let ty = Type::Var(self.fresh_tyvar());
                let tree = InferenceTree::new(RuleName::TPatWild, Input::Pat("_".into()), ty.clone(), vec![]);
                (ty, vec![], tree)
            }
            PatKind::Var(ident) => {
                let ty = Type::Var(self.fresh_tyvar());
                let scheme = Scheme {
                    type_vars: vec![],
                    ty: ty.clone(),
                };
                let name = self.rodeo.resolve(&ident.0).to_owned();
                let tree = InferenceTree::new(RuleName::TPatVar, Input::Pat(name), ty.clone(), vec![]);
                (ty, vec![(*ident, scheme)], tree)
            }
            PatKind::Lit(lit) => {
                let ty = match lit {
                    Literal::Int(_) => Type::Int,
                    Literal::Float(_) => Type::Float,
                    Literal::Bool(_) => Type::Bool,
                };
                let lit_str = match lit {
                    Literal::Int(n) => n.to_string(),
                    Literal::Float(f) => f.to_string(),
                    Literal::Bool(b) => b.to_string(),
                };
                let tree = InferenceTree::new(RuleName::TPatLit, Input::Pat(lit_str), ty.clone(), vec![]);
                (ty, vec![], tree)
            }
            PatKind::Or(a, b) => {
                let (ty_a, mut bindings, tree_a) = self.infer_pat(a);
                let (ty_b, bindings_b, tree_b) = self.infer_pat(b);
                let (s, unify_tree) = self.unify(&ty_a, &ty_b, pat.span);
                let unified = Self::apply_type(&s, &ty_a);
                bindings.extend(bindings_b.into_iter().map(|(id, scheme)| {
                    (
                        id,
                        Scheme {
                            type_vars: scheme.type_vars,
                            ty: Self::apply_type(&s, &scheme.ty),
                        },
                    )
                }));
                let a_display = match &tree_a.input {
                    Input::Pat(s) => s.clone(),
                    _ => unreachable!(),
                };
                let b_display = match &tree_b.input {
                    Input::Pat(s) => s.clone(),
                    _ => unreachable!(),
                };
                let combined = format!("{a_display} | {b_display}");
                let tree = InferenceTree::new(
                    RuleName::TPatOr,
                    Input::Pat(combined),
                    unified.clone(),
                    vec![tree_a, tree_b, unify_tree],
                );
                (unified, bindings, tree)
            }
        }
    }

    fn infer_match<'a>(
        &mut self,
        env: &Env,
        expr: &'a Expr<'a>,
        scrutinee: &'a Expr<'a>,
        arms: &'a [(Pat<'a>, Expr<'a>)],
    ) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);

        let (s1, scrut_ty, tree1) = self.infer(env, scrutinee);
        let mut final_subst = s1;
        let mut arm_trees: Vec<InferenceTree<'a>> = vec![tree1];
        let mut body_ty_opt = None;

        for (pat, body) in arms.iter() {
            let (pat_ty, bindings, pat_tree) = self.infer_pat(pat);

            let pat_ty_s = Self::apply_type(&final_subst, &pat_ty);
            let scrut_ty_s = Self::apply_type(&final_subst, &scrut_ty);
            let (s_pat, pat_uni_tree) = self.unify(&pat_ty_s, &scrut_ty_s, scrutinee.span);
            final_subst = s_pat.compose(&final_subst);
            arm_trees.push(pat_tree);
            arm_trees.push(pat_uni_tree);

            let env_arm = {
                let mut env = Self::apply_env(&final_subst, env);
                for (name, scheme) in &bindings {
                    let scheme = Self::apply_scheme(&final_subst, scheme);
                    env.insert(*name, scheme);
                }
                env
            };

            let (s_body, this_body_ty, body_tree) = self.infer(&env_arm, body);
            final_subst = s_body.compose(&final_subst);
            arm_trees.push(body_tree);

            match body_ty_opt {
                Some(ref prev_ty) => {
                    let prev_ty_s = Self::apply_type(&final_subst, prev_ty);
                    let this_ty_s = Self::apply_type(&final_subst, &this_body_ty);
                    let (s_uni, uni_tree) = self.unify(&prev_ty_s, &this_ty_s, body.span);
                    final_subst = s_uni.compose(&final_subst);
                    arm_trees.push(uni_tree);
                }
                None => {
                    body_ty_opt = Some(this_body_ty);
                }
            }
        }

        let final_ty = match body_ty_opt {
            Some(ty) => Self::apply_type(&final_subst, &ty),
            None => {
                self.emit_error(InferenceError::NoMatchArms { span: expr.span });
                let tree = InferenceTree::new(
                    RuleName::TMatch,
                    Input::Infer {
                        env: env_str,
                        expr,
                    },
                    Type::Never,
                    arm_trees,
                );
                return (final_subst, Type::Never, tree);
            }
        };

        let tree = InferenceTree::new(
            RuleName::TMatch,
            Input::Infer {
                env: env_str,
                expr,
            },
            final_ty.clone(),
            arm_trees,
        );
        (final_subst, final_ty, tree)
    }

    fn infer_lit<'a>(
        &mut self,
        env: &Env,
        expr: &'a Expr<'a>,
        literal: &Literal,
    ) -> (Subst, Type, InferenceTree<'a>) {
        let env_str = self.pretty_env(env);
        let (rule, output, ty) = match literal {
            Literal::Int(_) => (RuleName::TInt, "Int", Type::Int),
            Literal::Float(_) => (RuleName::TFloat, "Float", Type::Float),
            Literal::Bool(_) => (RuleName::TBool, "Bool", Type::Bool),
        };
        let tree = InferenceTree::new(
            rule,
            Input::Infer {
                env: env_str,
                expr,
            },
            output,
            vec![],
        );
        (Subst::empty(), ty, tree)
    }

    fn pretty_subst(&self, subst: &Subst) -> String {
        if subst.types.is_empty() {
            "{}".to_string()
        } else {
            format!(
                "{{{}}}",
                subst
                    .types
                    .iter()
                    .format_with(", ", |(k, v), f| f(&format_args!("{}/{}", v, k)))
            )
        }
    }

    fn pretty_env(&self, env: &Env) -> String {
        if env.is_empty() {
            "{}".to_string()
        } else {
            format!(
                "{{{}}}",
                env.iter()
                    .format_with(", ", |(k, v), f| f(&format_args!("{:?}: {}", k, v)))
            )
        }
    }

    fn occurs_check(&self, var: &TyVar, ty: &Type) -> bool {
        let mut fv = FreeVars::default();
        ftv_type(ty, &mut fv);
        fv.types.contains(var)
    }

    pub fn new(rodeo: &'rodeo mut Rodeo) -> Self {
        Self {
            counter: 0,
            rodeo,
            errors: Vec::new(),
        }
    }
}

fn ftv_type(ty: &Type, out: &mut FreeVars) {
    match ty {
        Type::Var(n) => {
            out.types.insert(n.clone());
        }
        Type::Arrow(a, b) => {
            ftv_type(a, out);
            ftv_type(b, out);
        }
        Type::Tuple(types) => {
            for t in types {
                ftv_type(t, out);
            }
        }
        Type::Int | Type::Bool | Type::Float => {}
        Type::Error | Type::Never => {}
    }
}

fn ftv_scheme(s: &Scheme) -> FreeVars {
    let mut fv = FreeVars::default();
    ftv_type(&s.ty, &mut fv);
    for v in &s.type_vars {
        fv.types.remove(v);
    }
    fv
}

fn ftv_env(env: &Env) -> FreeVars {
    let mut fv = FreeVars::default();
    for s in env.values() {
        let s_fv = ftv_scheme(s);
        fv.types.extend(s_fv.types);
    }
    fv
}

fn rename_scheme(s: &Scheme) -> Type {
    let mut subst = Subst::empty();
    let mut letters = ('a'..='z')
        .map(|c| c as u8 - 'a' as u8)
        .map(|v| TypeVar(v as u64));
    for v in &s.type_vars {
        let fresh = letters.next().unwrap_or_else(|| v.clone());
        subst.types.insert(v.clone(), Type::Var(fresh));
    }
    TypeInference::apply_type(&subst, &s.ty)
}

pub fn typecheck<'a>(expr: &'a Expr<'a>, rodeo: &mut Rodeo) -> TypeCheckResult<'a> {
    let mut inference = TypeInference::new(rodeo);
    let env = Env::new();
    let (s, ty, tree) = inference.infer(&env, expr);
    let final_ty = TypeInference::apply_type(&s, &ty);
    let scheme = inference.generalize(&env, &final_ty);
    TypeCheckResult {
        ty: rename_scheme(&scheme),
        errors: inference.take_errors(),
        tree,
    }
}

pub fn run_inference<'a>(expr: &'a Expr<'a>, rodeo: &mut Rodeo) -> Result<InferenceTree<'a>, ()> {
    let result = typecheck(expr, rodeo);
    if result.errors.is_empty() {
        Ok(result.tree)
    } else {
        Err(())
    }
}

pub fn infer_type(expr: &Expr, rodeo: &mut Rodeo) -> Result<Type, ()> {
    let result = typecheck(expr, rodeo);
    if result.errors.is_empty() {
        Ok(result.ty)
    } else {
        Err(())
    }
}
