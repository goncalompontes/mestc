use std::collections::{BTreeMap, HashMap};

use im::HashSet;
use itertools::Itertools;
use lasso::Rodeo;

use crate::{
    ast::{BinOp, Expr, ExprKind, Ident, Literal, Pat, UnaryOp},
    hir::{Type, TypeVar},
};

// Type schemes for polymorphic types
#[derive(Debug, Clone, PartialEq)]
pub struct Scheme {
    pub type_vars: Vec<TyVar>, // Quantified type variables
    pub ty: Type,              // The type being quantified over
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

pub type TyVar = TypeVar;
pub type TmVar = Ident;
pub type Env = BTreeMap<TmVar, Scheme>;

#[derive(Default)]
struct FreeVars {
    types: HashSet<TyVar>,
}

#[derive(Debug, Clone)]
pub struct Subst {
    pub types: HashMap<TyVar, Type>,
}

impl Subst {
    // Given two substitutions, we apply all of our substitutions to each of the others
    // substitutions, adding all of our substitutions into the result returning a composed substitution
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

    // A substitution with initialized with the provided substitution
    fn singleton_type(var: TyVar, ty: Type) -> Self {
        let mut s = Self::empty();
        s.types.insert(var, ty);
        s
    }

    // An empty substitution
    fn empty() -> Subst {
        Subst {
            types: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct InferenceTree {
    pub rule: String,
    pub input: String,
    pub output: String,
    pub children: Vec<InferenceTree>,
}

impl InferenceTree {
    pub fn new(
        rule: impl Into<String>,
        input: impl Into<String>,
        output: impl Into<String>,
        children: Vec<InferenceTree>,
    ) -> Self {
        Self {
            rule: rule.into(),
            input: input.into(),
            output: output.into(),
            children,
        }
    }

    pub fn display_tree(&self) -> String {
        let mut buf = String::new();
        self.write(&mut buf, &[]);
        buf
    }

    fn write(&self, buf: &mut String, prefix: &[bool]) {
        use std::fmt::Write;

        for &is_last in prefix.iter().take(prefix.len().saturating_sub(1)) {
            if is_last {
                buf.push_str("    ");
            } else {
                buf.push_str("│   ");
            }
        }
        if let Some(&last) = prefix.last() {
            if last {
                buf.push_str("└── ");
            } else {
                buf.push_str("├── ");
            }
        }

        writeln!(buf, "{} {} {}", self.rule, self.input, self.output).unwrap();

        for (i, child) in self.children.iter().enumerate() {
            let is_last = i == self.children.len() - 1;
            let mut child_prefix = prefix.to_vec();
            child_prefix.push(is_last);
            child.write(buf, &child_prefix);
        }
    }
}

pub struct TypeInference<'rodeo> {
    counter: u64,
    rodeo: &'rodeo mut Rodeo,
}

impl<'rodeo> TypeInference<'rodeo> {
    // Gives un a new unique type variable
    fn fresh_tyvar(&mut self) -> TyVar {
        let var = self.counter;
        self.counter += 1;
        TypeVar(var)
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

    // given a type with TypeVariables we want to apply a list of substitutions in that type
    // such that we specialize the type (the final type can may or may not be a concrete one *yet*)
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
        }
    }

    // given a scheme, or in other words, an abstraction, we want to remove any type variables
    // that shows up in the abstraction so that they are not substituted in inner applications
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

    // given an environment, we apply a substitution to all of it's schemes
    fn apply_env(s: &Subst, env: &Env) -> Env {
        env.iter()
            .map(|(k, scheme)| (k.clone(), Self::apply_scheme(s, scheme)))
            .collect()
    }

    fn unify(&self, this: &Type, other: &Type) -> Result<(Subst, InferenceTree), ()> {
        let input = format!("{} ~ {}", this, other);

        match (this, other) {
            // if types are the same we have no substitutions to make
            (Type::Int, Type::Int) | (Type::Float, Type::Float) | (Type::Bool, Type::Bool) => {
                let tree = InferenceTree::new("Unify-Base", &input, "{}", vec![]);
                Ok((Subst::empty(), tree))
            }
            (Type::Var(var), ty) | (ty, Type::Var(var)) => {
                if ty == &Type::Var(var.clone()) {
                    // if both are the same type variable then we have no substitutions
                    let tree = InferenceTree::new("Unify-Var-Same", &input, "{}", vec![]);
                    Ok((Subst::empty(), tree))
                } else if self.occurs_check(var, ty) {
                    // if the type variable occurs in the type then that's an error (recursive type definition)
                    Err(()) // replace with better errors
                } else {
                    // we have a typevar and type so we can assign the type to that type variable
                    let subst = Subst::singleton_type(var.clone(), ty.clone());
                    let output = format!("{{{}/{}}}", ty, var);
                    let tree = InferenceTree::new("Unfiy-Var", &input, &output, vec![]);
                    Ok((subst, tree))
                }
            }
            (Type::Arrow(from_a, to_a), Type::Arrow(from_b, to_b)) => {
                // first we unify both `from` types
                let (from, from_tree) = self.unify(from_a, from_b)?;
                // then we unify both the `to` types after applying the substitutions that resulted from unifying from
                let (to, to_tree) = self.unify(
                    &Self::apply_type(&from, to_a),
                    &Self::apply_type(&from, to_b),
                )?;
                // then we just compose the substitutions together
                let final_subst = to.compose(&from);
                let output = self.pretty_subst(&final_subst);
                let tree =
                    InferenceTree::new("Unify-Arrow", &input, &output, vec![from_tree, to_tree]);
                Ok((final_subst, tree))
            }
            (Type::Tuple(ts1), Type::Tuple(ts2)) => {
                // if their length doesn't match then we can exit early since know the types cannot possibly match
                if ts1.len() != ts2.len() {
                    return Err(()); // change to better types
                };

                let mut subst = Subst::empty();
                let mut trees = Vec::new();

                // for each pair of types in the tuple types, try unifying them, after applying the substitutions that resulted from previous unifications
                for (t1, t2) in ts1.iter().zip(ts2.iter()) {
                    let (s, tree) =
                        self.unify(&Self::apply_type(&subst, t1), &Self::apply_type(&subst, t2))?;
                    subst = s.compose(&subst);
                    trees.push(tree);
                }

                let output = self.pretty_subst(&subst);
                let tree = InferenceTree::new("Unify-Tuple", &input, &output, trees);
                Ok((subst, tree))
            }
            _ => Err(()), // change to better types
        }
    }

    pub fn infer(&mut self, env: &Env, expr: &Expr) -> Result<(Subst, Type, InferenceTree), ()> {
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

    fn infer_var(
        &mut self,
        env: &Env,
        expr: &Expr,
        name: Ident,
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));
        match env.get(&name) {
            Some(scheme) => {
                let instantiated = self.instantiate(scheme);
                let output = format!("{}", instantiated);
                let tree = InferenceTree::new("T-Var", &input, &output, vec![]);
                Ok((Subst::empty(), instantiated, tree))
            }
            None => Err(()), // replace with proper errors
        }
    }

    fn infer_abs(
        &mut self,
        env: &Env,
        expr: &Expr,
        param: &TmVar,
        body: &Expr,
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));

        // create a new type variable for the param
        let param_ty = Type::Var(self.fresh_tyvar());
        let mut new_env = env.clone();
        // create a scheme of the abstraction
        let param_scheme = Scheme {
            type_vars: vec![],
            ty: param_ty.clone(),
        };
        // extend the env with the scheme
        new_env.insert(*param, param_scheme);

        // infer the body of the abstraction
        let (s1, body_ty, tree1) = self.infer(&new_env, body)?;
        // the result type is constructed by applying the substitution that resulted from infering the body
        // to the param type and creating an arrow from it to the infered body type
        let result_ty = Type::Arrow(
            Box::new(Self::apply_type(&s1, &param_ty)),
            Box::new(body_ty),
        );

        let output = format!("{}", result_ty);
        let tree = InferenceTree::new("T-Abs", &input, &output, vec![tree1]);
        Ok((s1, result_ty, tree))
    }

    fn infer_app(
        &mut self,
        env: &Env,
        expr: &Expr,
        func: &Expr,
        arg: &Expr,
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));

        let result_ty = Type::Var(self.fresh_tyvar());

        // we infer the abstraction applicative
        let (s1, func_ty, tree1) = self.infer(env, func)?;
        // then apply the resulting substitution to the env
        let env_subst = Self::apply_env(&s1, env);
        // we infer the argument applied
        let (s2, arg_ty, tree2) = self.infer(&env_subst, arg)?;

        // now that we infered the argument, we apply the substitutions from the arg in the abstraction
        // we get a type for the instantiated function type
        let func_ty_subst = Self::apply_type(&s2, &func_ty);
        // we also create the expected type for the abstraction (arg_ty -> result_ty)
        let expected_func_ty = Type::Arrow(Box::new(arg_ty), Box::new(result_ty.clone()));

        // we unify the expected type for the abstraction with the infered one
        let (s3, tree3) = self.unify(&func_ty_subst, &expected_func_ty)?;

        // we compose all substitutions together
        let final_subst = s3.compose(&s2.compose(&s1));
        // and derive a final type (the concrete result type) by applying the substitutions to the result type
        let final_ty = Self::apply_type(&s3, &result_ty);

        let output = format!("{}", final_ty);
        let tree = InferenceTree::new("T-App", &input, &output, vec![tree1, tree2, tree3]);
        Ok((final_subst, final_ty, tree))
    }

    fn infer_let(
        &mut self,
        env: &Env,
        expr: &Expr,
        var: &TmVar,
        value: &Expr,
        body: &Expr,
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));

        // we infer the value of the let binding
        let (s1, value_ty, tree1) = self.infer(env, value)?;
        // then we apply the substitution from the infered value to the env
        let env_subst = Self::apply_env(&s1, env);
        // and generalize the type of the value with the context of the env
        let generalized_ty = self.generalize(&env_subst, &value_ty);

        let mut new_env = env_subst;
        // we insert the binding to the env with the generalized type
        new_env.insert(var.clone(), generalized_ty);

        // we infer the body of the let binding (the expression the let binding is bound in)
        let (s2, body_ty, tree2) = self.infer(&new_env, body)?;

        // then finally we compose the substitutions
        let final_subst = s2.compose(&s1);
        let output = format!("{}", body_ty);
        let tree = InferenceTree::new("T-Let", &input, &output, vec![tree1, tree2]);
        Ok((final_subst, body_ty, tree))
    }

    fn infer_if(
        &mut self,
        env: &Env,
        expr: &Expr,
        cond: &Expr,
        then_expr: &Expr,
        else_expr: &Expr,
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));

        let (s1, cond_ty, tree1) = self.infer(env, cond)?;
        let env1 = Self::apply_env(&s1, env);
        let (s2, then_ty, tree2) = self.infer(&env1, then_expr)?;
        let env2 = Self::apply_env(&s2, &env1);
        let (s3, else_ty, tree3) = self.infer(&env2, else_expr)?;

        let cond_ty_s = Self::apply_type(&s3, &Self::apply_type(&s2, &cond_ty));
        let (s4, cond_tree) = self.unify(&cond_ty_s, &Type::Bool)?;

        let then_ty_s = Self::apply_type(&s4, &Self::apply_type(&s3, &then_ty));
        let else_ty_s = Self::apply_type(&s4, &else_ty);
        let (s5, eq_tree) = self.unify(&then_ty_s, &else_ty_s)?;

        let final_subst = s5.compose(&s4.compose(&s3.compose(&s2.compose(&s1))));
        let final_ty = Self::apply_type(&final_subst, &then_ty);

        let output = format!("{}", final_ty);
        let tree = InferenceTree::new(
            "T-If",
            &input,
            &output,
            vec![tree1, tree2, tree3, cond_tree, eq_tree],
        );
        Ok((final_subst, final_ty, tree))
    }

    fn infer_binop(
        &mut self,
        env: &Env,
        expr: &Expr,
        op: &BinOp,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));

        let (expected_lhs_ty, expected_rhs_ty, result_ty) = match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow => {
                (Type::Int, Type::Int, Type::Int)
            }
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                (Type::Int, Type::Int, Type::Bool)
            }
            BinOp::And | BinOp::Or => (Type::Bool, Type::Bool, Type::Bool),
        };

        let (s1, lhs_ty, tree1) = self.infer(env, lhs)?;
        let env1 = Self::apply_env(&s1, env);
        let (s2, rhs_ty, tree2) = self.infer(&env1, rhs)?;

        let lhs_ty_s = Self::apply_type(&s2, &lhs_ty);
        let (s3, lhs_tree) = self.unify(&lhs_ty_s, &expected_lhs_ty)?;

        let rhs_ty_s = Self::apply_type(&s3, &rhs_ty);
        let (s4, rhs_tree) = self.unify(&rhs_ty_s, &expected_rhs_ty)?;

        let final_subst = s4.compose(&s3.compose(&s2.compose(&s1)));
        let final_ty = Self::apply_type(&final_subst, &result_ty);

        let output = format!("{}", final_ty);
        let tree = InferenceTree::new(
            "T-BinOp",
            &input,
            &output,
            vec![tree1, tree2, lhs_tree, rhs_tree],
        );
        Ok((final_subst, final_ty, tree))
    }

    fn infer_unaryop(
        &mut self,
        env: &Env,
        expr: &Expr,
        op: &UnaryOp,
        rhs: &Expr,
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));

        let (expected_arg_ty, result_ty) = match op {
            UnaryOp::Neg => (Type::Int, Type::Int),
            UnaryOp::Not => (Type::Bool, Type::Bool),
        };

        let (s1, rhs_ty, tree1) = self.infer(env, rhs)?;
        let rhs_ty_s = Self::apply_type(&s1, &rhs_ty);
        let (s2, rhs_tree) = self.unify(&rhs_ty_s, &expected_arg_ty)?;

        let final_subst = s2.compose(&s1);
        let final_ty = Self::apply_type(&final_subst, &result_ty);

        let output = format!("{}", final_ty);
        let tree = InferenceTree::new("T-UnaryOp", &input, &output, vec![tree1, rhs_tree]);
        Ok((final_subst, final_ty, tree))
    }

    fn infer_pat(&mut self, pat: &Pat) -> (Type, Vec<(Ident, Scheme)>) {
        match pat {
            Pat::Wildcard => (Type::Var(self.fresh_tyvar()), vec![]),
            Pat::Var(ident) => {
                let ty = Type::Var(self.fresh_tyvar());
                let scheme = Scheme {
                    type_vars: vec![],
                    ty: ty.clone(),
                };
                (ty, vec![(*ident, scheme)])
            }
            Pat::Lit(lit) => {
                let ty = match lit {
                    Literal::Int(_) => Type::Int,
                    Literal::Float(_) => Type::Float,
                    Literal::Bool(_) => Type::Bool,
                };
                (ty, vec![])
            }
            Pat::Or(a, b) => {
                let (ty_a, mut bindings) = self.infer_pat(a);
                let (ty_b, bindings_b) = self.infer_pat(b);
                let (s, _) = self
                    .unify(&ty_a, &ty_b)
                    .unwrap_or((Subst::empty(), InferenceTree::new("", "", "", vec![])));
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
                (unified, bindings)
            }
        }
    }

    fn infer_match(
        &mut self,
        env: &Env,
        expr: &Expr,
        scrutinee: &Expr,
        arms: &[(Pat, Expr)],
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));

        let (s1, scrut_ty, tree1) = self.infer(env, scrutinee)?;
        let mut final_subst = s1;
        let mut arm_trees = vec![tree1];
        let mut body_ty_opt = None;

        for (pat, body) in arms.iter() {
            let (pat_ty, bindings) = self.infer_pat(pat);

            let pat_ty_s = Self::apply_type(&final_subst, &pat_ty);
            let scrut_ty_s = Self::apply_type(&final_subst, &scrut_ty);
            let (s_pat, pat_tree) = self.unify(&pat_ty_s, &scrut_ty_s)?;
            final_subst = s_pat.compose(&final_subst);
            arm_trees.push(pat_tree);

            let env_arm = {
                let mut env = Self::apply_env(&final_subst, env);
                for (name, scheme) in &bindings {
                    let scheme = Self::apply_scheme(&final_subst, scheme);
                    env.insert(*name, scheme);
                }
                env
            };

            let (s_body, this_body_ty, body_tree) = self.infer(&env_arm, body)?;
            final_subst = s_body.compose(&final_subst);
            arm_trees.push(body_tree);

            match body_ty_opt {
                Some(ref prev_ty) => {
                    let prev_ty_s = Self::apply_type(&final_subst, prev_ty);
                    let this_ty_s = Self::apply_type(&final_subst, &this_body_ty);
                    let (s_uni, uni_tree) = self.unify(&prev_ty_s, &this_ty_s)?;
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
            None => return Err(()),
        };

        let output = format!("{}", final_ty);
        let tree = InferenceTree::new("T-Match", &input, &output, arm_trees);
        Ok((final_subst, final_ty, tree))
    }

    fn infer_lit(
        &mut self,
        env: &Env,
        expr: &Expr,
        literal: &Literal,
    ) -> Result<(Subst, Type, InferenceTree), ()> {
        let input = format!("{} ⊢ {} ⇒", self.pretty_env(env), expr.display(self.rodeo));
        let (rule, output, ty) = match literal {
            Literal::Int(_) => ("T-Int", "Int", Type::Int),
            Literal::Float(_) => ("T-Float", "Float", Type::Float),
            Literal::Bool(_) => ("T-Bool", "Bool", Type::Bool),
        };
        let tree = InferenceTree::new(rule, input, output, vec![]);
        Ok((Subst::empty(), ty, tree))
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

    // check if a type variable occurs within a type
    fn occurs_check(&self, var: &TyVar, ty: &Type) -> bool {
        let mut fv = FreeVars::default();
        ftv_type(ty, &mut fv);
        fv.types.contains(var)
    }

    pub fn new(rodeo: &'rodeo mut Rodeo) -> Self {
        Self { counter: 0, rodeo }
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

pub fn run_inference(expr: &Expr, rodeo: &mut Rodeo) -> Result<InferenceTree, ()> {
    let mut inference = TypeInference::new(rodeo);
    let env = BTreeMap::new();
    let (_, _, tree) = inference.infer(&env, expr)?;
    Ok(tree)
}

pub fn infer_type(expr: &Expr, rodeo: &mut Rodeo) -> Result<Type, ()> {
    let mut inference = TypeInference::new(rodeo);
    let env = Env::new();
    let (s, ty, _) = inference.infer(&env, expr)?;
    let final_ty = TypeInference::apply_type(&s, &ty);
    let scheme = inference.generalize(&env, &final_ty);
    Ok(rename_scheme(&scheme))
}
