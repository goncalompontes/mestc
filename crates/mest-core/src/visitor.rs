use crate::ast::{
    BinOp, BindPat, BindPatKind, Binding, Expr, ExprKind, Literal, Pat, PatKind, UnaryOp,
};
use lasso::Rodeo;

pub trait Visitor<'bump, Ctx>: Sized {
    fn visit_expr(&mut self, _expr: &'bump Expr<'bump>, _ctx: &mut Ctx) {}
    fn visit_pat(&mut self, _pat: &'bump Pat<'bump>, _ctx: &mut Ctx) {}
    fn visit_binding(&mut self, _binding: &'bump Binding<'bump>, _ctx: &mut Ctx) {}
    fn visit_bind_pat(&mut self, _pat: &'bump BindPat<'bump>, _ctx: &mut Ctx) {}

    fn walk_expr(&mut self, expr: &'bump Expr<'bump>, ctx: &mut Ctx) {
        self.visit_expr(expr, ctx);
        match &*expr.kind {
            ExprKind::Literal(_) | ExprKind::Var(_) => {}
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                self.walk_expr(cond, ctx);
                self.walk_expr(then_expr, ctx);
                self.walk_expr(else_expr, ctx);
            }
            ExprKind::BinOp { op: _, lhs, rhs } => {
                self.walk_expr(lhs, ctx);
                self.walk_expr(rhs, ctx);
            }
            ExprKind::UnaryOp { op: _, rhs } => {
                self.walk_expr(rhs, ctx);
            }
            ExprKind::Let { bindings, body, .. } => {
                for binding in bindings.iter() {
                    self.walk_binding(binding, ctx);
                }
                self.walk_expr(body, ctx);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.walk_expr(scrutinee, ctx);
                for (pat, body) in arms.iter() {
                    self.walk_pat(pat, ctx);
                    self.walk_expr(body, ctx);
                }
            }
            ExprKind::Abs { param, body } => {
                self.walk_pat(param, ctx);
                self.walk_expr(body, ctx);
            }
            ExprKind::App { func, arg } => {
                self.walk_expr(func, ctx);
                self.walk_expr(arg, ctx);
            }
            ExprKind::Tuple(items) => {
                for item in items.iter() {
                    self.walk_expr(item, ctx);
                }
            }
            ExprKind::Type { body, .. } => self.walk_expr(body, ctx),
            ExprKind::Constructor { arg, .. } => {
                if let Some(arg) = arg {
                    self.walk_expr(arg, ctx);
                }
            }
        }
    }

    fn walk_pat(&mut self, pat: &'bump Pat<'bump>, ctx: &mut Ctx) {
        self.visit_pat(pat, ctx);
        match &**pat {
            PatKind::Wildcard | PatKind::Var(_) | PatKind::Lit(_) => {}
            PatKind::Or(a, b) => {
                self.walk_pat(a, ctx);
                self.walk_pat(b, ctx);
            }
            PatKind::Tuple(pats) => {
                for p in pats.iter() {
                    self.walk_pat(p, ctx);
                }
            }
            PatKind::Constructor { arg, .. } => {
                if let Some(arg) = arg {
                    self.walk_pat(arg, ctx);
                }
            }
        }
    }

    fn walk_binding(&mut self, binding: &'bump Binding<'bump>, ctx: &mut Ctx) {
        self.visit_binding(binding, ctx);
        self.walk_bind_pat(&binding.pat, ctx);
        self.walk_expr(&binding.value, ctx);
    }

    fn walk_bind_pat(&mut self, pat: &'bump BindPat<'bump>, ctx: &mut Ctx) {
        self.visit_bind_pat(pat, ctx);
        match pat.kind {
            BindPatKind::Tuple(bind_pats) => {
                bind_pats
                    .iter()
                    .for_each(|pat| self.walk_bind_pat(pat, ctx));
            }
            _ => {}
        }
    }
}

// TODO: VisitMut requires `kind: &'bump mut ExprKind` on `Expr` to walk children.
// Add once the AST supports mutable node access.

pub struct PrintCtx<'a> {
    pub rodeo: &'a Rodeo,
    pub indent: usize,
    pub output: String,
}

impl<'a> PrintCtx<'a> {
    pub fn new(rodeo: &'a Rodeo) -> Self {
        Self {
            rodeo,
            indent: 0,
            output: String::new(),
        }
    }
}

pub struct AstPrinter;

pub struct DisplayExpr<'a, 'bump> {
    expr: &'bump Expr<'bump>,
    rodeo: &'a Rodeo,
}

impl std::fmt::Display for DisplayExpr<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ctx = PrintCtx::new(self.rodeo);
        AstPrinter.visit_expr(self.expr, &mut ctx);
        f.write_str(&ctx.output)
    }
}

impl AstPrinter {
    pub fn print_expr<'bump, 'a>(
        expr: &'bump Expr<'bump>,
        rodeo: &'a Rodeo,
    ) -> DisplayExpr<'a, 'bump> {
        DisplayExpr { expr, rodeo }
    }
}

impl<'bump> Expr<'bump> {
    pub fn display<'a>(&'bump self, rodeo: &'a Rodeo) -> DisplayExpr<'a, 'bump> {
        DisplayExpr { expr: self, rodeo }
    }

    pub fn display_inline<'a>(&'bump self, rodeo: &'a Rodeo) -> InlineDisplay<'a, 'bump> {
        InlineDisplay { expr: self, rodeo }
    }

    pub fn display_minimized<'a>(&'bump self, rodeo: &'a Rodeo) -> SimplifiedDisplay<'a, 'bump> {
        SimplifiedDisplay { expr: self, rodeo }
    }
}

pub struct SimplifiedDisplay<'a, 'bump> {
    expr: &'bump Expr<'bump>,
    rodeo: &'a Rodeo,
}

impl std::fmt::Display for SimplifiedDisplay<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_minimized(f, &**self.expr, self.rodeo)
    }
}

fn indent(level: usize) -> String {
    "  ".repeat(level)
}

pub struct InlineDisplay<'a, 'bump> {
    expr: &'bump Expr<'bump>,
    rodeo: &'a Rodeo,
}

impl std::fmt::Display for InlineDisplay<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_inline(f, &**self.expr, self.rodeo)
    }
}

fn write_inline(f: &mut std::fmt::Formatter, kind: &ExprKind, rodeo: &Rodeo) -> std::fmt::Result {
    match kind {
        ExprKind::Literal(lit) => match lit {
            Literal::Int(n) => write!(f, "{n}"),
            Literal::Float(v) => write!(f, "{v}"),
            Literal::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
        },
        ExprKind::Var(ident) => f.write_str(rodeo.resolve(&ident.name)),
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => {
            write!(f, "if ")?;
            write_inline(f, &**cond, rodeo)?;
            write!(f, " then ")?;
            write_inline(f, &**then_expr, rodeo)?;
            write!(f, " else ")?;
            write_inline(f, &**else_expr, rodeo)
        }
        ExprKind::BinOp { op, lhs, rhs } => {
            let op_str = match op {
                BinOp::Eq => " == ",
                BinOp::NotEq => " != ",
                BinOp::Lt => " < ",
                BinOp::Gt => " > ",
                BinOp::Le => " <= ",
                BinOp::Ge => " >= ",
                BinOp::And => " && ",
                BinOp::Or => " || ",
                BinOp::Add => " + ",
                BinOp::Sub => " - ",
                BinOp::Mul => " * ",
                BinOp::Div => " / ",
                BinOp::Pow => " ^ ",
            };
            parexpr_inline(lhs, op, rodeo, f)?;
            f.write_str(op_str)?;
            parexpr_inline(rhs, op, rodeo, f)
        }
        ExprKind::UnaryOp { op, rhs } => {
            let op_str = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            f.write_str(op_str)?;
            write_inline(f, &**rhs, rodeo)
        }
        ExprKind::Let { bindings, body } => {
            let rec = bindings.first().map_or(false, |b| b.rec);
            if rec {
                write!(f, "let rec ")?;
            } else {
                write!(f, "let ")?;
            }
            for (i, binding) in bindings.iter().enumerate() {
                if i > 0 {
                    write!(f, " and ")?;
                }
                write_bind_pat(f, &binding.pat, rodeo)?;
                write!(f, " = ")?;
                write_inline(f, &*binding.value.kind, rodeo)?;
            }
            write!(f, " in ")?;
            write_inline(f, &*body.kind, rodeo)
        }
        ExprKind::Match { scrutinee, arms } => {
            write!(f, "match ")?;
            write_inline(f, &**scrutinee, rodeo)?;
            for (pat, body) in arms.iter() {
                write!(f, " | ")?;
                pat_write_inline(pat, rodeo, f)?;
                write!(f, " => ")?;
                write_inline(f, &**body, rodeo)?;
            }
            Ok(())
        }
        ExprKind::Abs { param, body } => {
            write!(f, "|")?;
            pat_write_inline(param, rodeo, f)?;
            write!(f, "| ")?;
            write_inline(f, &**body, rodeo)
        }
        ExprKind::App { func, arg } => {
            let func_need_paren = matches!(
                &**func,
                ExprKind::Abs { .. }
                    | ExprKind::If { .. }
                    | ExprKind::Let { .. }
                    | ExprKind::Match { .. }
            );
            if func_need_paren {
                write!(f, "(")?;
                write_inline(f, &**func, rodeo)?;
                write!(f, ")")?;
            } else {
                write_inline(f, &**func, rodeo)?;
            }
            write!(f, " ")?;
            let arg_need_paren = matches!(
                &**arg,
                ExprKind::Abs { .. }
                    | ExprKind::If { .. }
                    | ExprKind::Let { .. }
                    | ExprKind::Match { .. }
            );
            if arg_need_paren {
                write!(f, "(")?;
                write_inline(f, &**arg, rodeo)?;
                write!(f, ")")?;
            } else {
                write_inline(f, &**arg, rodeo)?;
            }
            Ok(())
        }
        ExprKind::Tuple(items) => {
            write!(f, "(")?;
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write_inline(f, &**item, rodeo)?;
            }
            write!(f, ")")
        }
        ExprKind::Type { body, .. } => {
            write!(f, "[type ...] ")?;
            write_inline(f, &**body, rodeo)
        }
        ExprKind::Constructor { name, arg } => {
            write!(f, "{}", rodeo.resolve(&name.name))?;
            if let Some(arg) = arg {
                write!(f, " ")?;
                write_inline(f, &**arg, rodeo)?;
            }
            Ok(())
        }
    }
}

fn write_bind_pat(f: &mut std::fmt::Formatter, pat: &BindPat, rodeo: &Rodeo) -> std::fmt::Result {
    match pat.kind {
        BindPatKind::Var(ident) => f.write_str(rodeo.resolve(&ident.name)),
        BindPatKind::Tuple(pats) => {
            write!(f, "(")?;
            for (i, p) in pats.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write_bind_pat(f, p, rodeo)?;
            }
            write!(f, ")")
        }
    }
}

fn parexpr_inline(
    expr: &Expr,
    parent_op: &BinOp,
    rodeo: &Rodeo,
    f: &mut std::fmt::Formatter,
) -> std::fmt::Result {
    match &**expr {
        ExprKind::BinOp { op: child_op, .. } if binop_prec(child_op) < binop_prec(parent_op) => {
            write!(f, "(")?;
            write_inline(f, &**expr, rodeo)?;
            write!(f, ")")
        }
        _ => write_inline(f, &**expr, rodeo),
    }
}

fn pat_write_inline(pat: &Pat, rodeo: &Rodeo, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match &**pat {
        PatKind::Wildcard => f.write_str("_"),
        PatKind::Var(ident) => f.write_str(rodeo.resolve(&ident.name)),
        PatKind::Lit(lit) => match lit {
            Literal::Int(n) => write!(f, "{n}"),
            Literal::Float(v) => write!(f, "{v}"),
            Literal::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
        },
        PatKind::Or(a, b) => {
            pat_write_inline(a, rodeo, f)?;
            write!(f, " | ")?;
            pat_write_inline(b, rodeo, f)
        }
        PatKind::Tuple(pats) => {
            write!(f, "(")?;
            for (i, p) in pats.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                pat_write_inline(p, rodeo, f)?;
            }
            write!(f, ")")
        }
        PatKind::Constructor { name, arg } => {
            write!(f, "{}", rodeo.resolve(&name.name))?;
            if let Some(arg) = arg {
                write!(f, " (")?;
                pat_write_inline(arg, rodeo, f)?;
                write!(f, ")")?;
            }
            Ok(())
        }
    }
}

fn write_minimized(
    f: &mut std::fmt::Formatter,
    kind: &ExprKind,
    rodeo: &Rodeo,
) -> std::fmt::Result {
    match kind {
        ExprKind::Literal(lit) => match lit {
            Literal::Int(n) => write!(f, "{n}"),
            Literal::Float(v) => write!(f, "{v}"),
            Literal::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
        },
        ExprKind::Var(ident) => f.write_str(rodeo.resolve(&ident.name)),
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => {
            write!(f, "if ")?;
            write_minimized(f, &**cond, rodeo)?;
            write!(f, " then ")?;
            write_minimized(f, &**then_expr, rodeo)?;
            write!(f, " else ")?;
            write_minimized(f, &**else_expr, rodeo)
        }
        ExprKind::BinOp { op, lhs, rhs } => {
            write_minimized(f, &**lhs, rodeo)?;
            let op_str = match op {
                BinOp::Eq => " == ",
                BinOp::NotEq => " != ",
                BinOp::Lt => " < ",
                BinOp::Gt => " > ",
                BinOp::Le => " <= ",
                BinOp::Ge => " >= ",
                BinOp::And => " && ",
                BinOp::Or => " || ",
                BinOp::Add => " + ",
                BinOp::Sub => " - ",
                BinOp::Mul => " * ",
                BinOp::Div => " / ",
                BinOp::Pow => " ^ ",
            };
            f.write_str(op_str)?;
            write_minimized(f, &**rhs, rodeo)
        }
        ExprKind::UnaryOp { op, rhs } => {
            match op {
                UnaryOp::Neg => f.write_str("-")?,
                UnaryOp::Not => f.write_str("!")?,
            }
            write_minimized(f, &**rhs, rodeo)
        }
        ExprKind::Let { bindings, .. } => {
            let rec = bindings.first().map_or(false, |b| b.rec);
            if rec {
                write!(f, "let rec ")?;
            } else {
                write!(f, "let ")?;
            }
            if let Some(binding) = bindings.first() {
                write_minimized(f, &*binding.value.kind, rodeo)?;
            }
            if bindings.len() > 1 {
                write!(f, " and ...")?;
            }
            write!(f, " in ...")
        }
        ExprKind::Match { scrutinee, arms } => {
            write!(f, "match ")?;
            write_minimized(f, &**scrutinee, rodeo)?;
            if let Some((pat, body)) = arms.first() {
                write!(f, " | ")?;
                pat_write_inline(pat, rodeo, f)?;
                write!(f, " => ")?;
                write_minimized(f, &**body, rodeo)?;
            }
            if arms.len() > 1 {
                write!(f, " | ...")?;
            }
            Ok(())
        }
        ExprKind::Abs { param, body } => {
            write!(f, "|")?;
            pat_write_inline(param, rodeo, f)?;
            write!(f, "| ")?;
            write_minimized(f, &**body, rodeo)
        }
        ExprKind::App { func, arg } => {
            write_minimized(f, &**func, rodeo)?;
            write!(f, " ")?;
            write_minimized(f, &**arg, rodeo)
        }
        ExprKind::Tuple(items) => {
            write!(f, "(")?;
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write_minimized(f, &**item, rodeo)?;
            }
            write!(f, ")")
        }
        ExprKind::Type { body, .. } => {
            write!(f, "[type ...] ")?;
            write_minimized(f, &**body, rodeo)
        }
        ExprKind::Constructor { name, arg } => {
            write!(f, "{}", rodeo.resolve(&name.name))?;
            if let Some(arg) = arg {
                write!(f, " ")?;
                write_minimized(f, &**arg, rodeo)?;
            }
            Ok(())
        }
    }
}

fn binop_prec(op: &BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div => 5,
        BinOp::Pow => 6,
    }
}

fn needs_parens_simple(expr: &Expr) -> bool {
    matches!(
        &*expr.kind,
        ExprKind::If { .. }
            | ExprKind::Let { .. }
            | ExprKind::Match { .. }
            | ExprKind::BinOp { .. }
            | ExprKind::UnaryOp { .. }
            | ExprKind::Type { .. }
            | ExprKind::Constructor { .. }
    )
}

fn parexpr<'bump>(
    visitor: &mut AstPrinter,
    expr: &'bump Expr<'bump>,
    ctx: &mut PrintCtx,
    wrap: bool,
) {
    if wrap && needs_parens_simple(expr) {
        ctx.output.push('(');
        visitor.visit_expr(expr, ctx);
        ctx.output.push(')');
    } else {
        visitor.visit_expr(expr, ctx);
    }
}

fn parexpr_binop<'bump>(
    visitor: &mut AstPrinter,
    child: &'bump Expr<'bump>,
    parent_op: &BinOp,
    ctx: &mut PrintCtx,
) {
    let needs_parens = match &*child.kind {
        ExprKind::BinOp { op: child_op, .. } => binop_prec(child_op) < binop_prec(parent_op),
        _ => needs_parens_simple(child),
    };
    if needs_parens {
        ctx.output.push('(');
        visitor.visit_expr(child, ctx);
        ctx.output.push(')');
    } else {
        visitor.visit_expr(child, ctx);
    }
}

impl<'bump> Visitor<'bump, PrintCtx<'_>> for AstPrinter {
    fn visit_expr(&mut self, expr: &'bump Expr<'bump>, ctx: &mut PrintCtx<'_>) {
        match &*expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(n) => ctx.output.push_str(&n.to_string()),
                Literal::Float(f) => ctx.output.push_str(&f.to_string()),
                Literal::Bool(b) => ctx.output.push_str(if *b { "true" } else { "false" }),
            },
            ExprKind::Var(ident) => {
                ctx.output.push_str(ctx.rodeo.resolve(&ident.name));
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                ctx.output.push_str("if ");
                self.visit_expr(cond, ctx);
                ctx.output.push('\n');
                ctx.output.push_str(&indent(ctx.indent));
                ctx.output.push_str("then\n");
                ctx.output.push_str(&indent(ctx.indent + 1));
                self.visit_expr(then_expr, ctx);
                ctx.output.push('\n');
                ctx.output.push_str(&indent(ctx.indent));
                ctx.output.push_str("else\n");
                ctx.output.push_str(&indent(ctx.indent + 1));
                self.visit_expr(else_expr, ctx);
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                let op_str = match op {
                    BinOp::Eq => " == ",
                    BinOp::NotEq => " != ",
                    BinOp::Lt => " < ",
                    BinOp::Gt => " > ",
                    BinOp::Le => " <= ",
                    BinOp::Ge => " >= ",
                    BinOp::And => " && ",
                    BinOp::Or => " || ",
                    BinOp::Add => " + ",
                    BinOp::Sub => " - ",
                    BinOp::Mul => " * ",
                    BinOp::Div => " / ",
                    BinOp::Pow => " ^ ",
                };
                parexpr_binop(self, lhs, op, ctx);
                ctx.output.push_str(op_str);
                parexpr_binop(self, rhs, op, ctx);
            }
            ExprKind::UnaryOp { op, rhs } => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                ctx.output.push_str(op_str);
                parexpr(self, rhs, ctx, true);
            }
            ExprKind::Let { bindings, body } => {
                let rec = bindings.first().map_or(false, |b| b.rec);
                if rec {
                    ctx.output.push_str("let rec ");
                } else {
                    ctx.output.push_str("let ");
                }
                for (i, binding) in bindings.iter().enumerate() {
                    if i > 0 {
                        ctx.output.push_str("and ");
                    }
                    push_bind_pat(&binding.pat, ctx);
                    ctx.output.push_str(" = ");
                    ctx.indent += 1;
                    self.visit_expr(&binding.value, ctx);
                    ctx.output.push('\n');
                    ctx.indent -= 1;
                    ctx.output.push_str(&indent(ctx.indent));
                }
                ctx.output.push_str("in\n");
                ctx.output.push_str(&indent(ctx.indent + 1));
                self.visit_expr(body, ctx);
            }
            ExprKind::Match { scrutinee, arms } => {
                ctx.output.push_str("match ");
                self.visit_expr(scrutinee, ctx);
                ctx.output.push('\n');
                for (i, (pat, body)) in arms.iter().enumerate() {
                    ctx.output.push_str(&indent(ctx.indent));
                    ctx.output.push_str("| ");
                    self.visit_pat(pat, ctx);
                    ctx.output.push_str(" => ");
                    self.visit_expr(body, ctx);
                    if i < arms.len() - 1 {
                        ctx.output.push('\n');
                    }
                }
            }
            ExprKind::Abs { param, body } => {
                ctx.output.push('|');
                self.visit_pat(param, ctx);
                ctx.output.push_str("| ");
                self.visit_expr(body, ctx);
            }
            ExprKind::App { func, arg } => {
                parexpr(self, func, ctx, true);
                ctx.output.push(' ');
                parexpr(self, arg, ctx, true);
            }
            ExprKind::Tuple(items) => {
                ctx.output.push('(');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        ctx.output.push_str(", ");
                    }
                    self.visit_expr(item, ctx);
                }
                ctx.output.push(')');
            }
            ExprKind::Type { body, .. } => {
                ctx.output.push_str("type ... in\n");
                ctx.output.push_str(&indent(ctx.indent + 1));
                self.visit_expr(body, ctx);
            }
            ExprKind::Constructor { name, arg } => {
                ctx.output.push_str(ctx.rodeo.resolve(&name.name));
                if let Some(arg) = arg {
                    ctx.output.push(' ');
                    self.visit_expr(arg, ctx);
                }
            }
        }
    }

    fn visit_pat(&mut self, pat: &'bump Pat<'bump>, ctx: &mut PrintCtx<'_>) {
        match &**pat {
            PatKind::Wildcard => ctx.output.push('_'),
            PatKind::Var(ident) => ctx.output.push_str(ctx.rodeo.resolve(&ident.name)),
            PatKind::Lit(lit) => match lit {
                Literal::Int(n) => ctx.output.push_str(&n.to_string()),
                Literal::Float(f) => ctx.output.push_str(&f.to_string()),
                Literal::Bool(b) => ctx.output.push_str(if *b { "true" } else { "false" }),
            },
            PatKind::Or(a, b) => {
                self.visit_pat(a, ctx);
                ctx.output.push_str(" | ");
                self.visit_pat(b, ctx);
            }
            PatKind::Tuple(pats) => {
                ctx.output.push('(');
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        ctx.output.push_str(", ");
                    }
                    self.visit_pat(p, ctx);
                }
                ctx.output.push(')');
            }
            PatKind::Constructor { name, arg } => {
                ctx.output.push_str(ctx.rodeo.resolve(&name.name));
                if let Some(arg) = arg {
                    ctx.output.push(' ');
                    self.visit_pat(arg, ctx);
                }
            }
        }
    }
}

fn push_bind_pat(pat: &BindPat, ctx: &mut PrintCtx) {
    match pat.kind {
        BindPatKind::Var(ident) => ctx.output.push_str(ctx.rodeo.resolve(&ident.name)),
        BindPatKind::Tuple(pats) => {
            ctx.output.push('(');
            for (i, p) in pats.iter().enumerate() {
                if i > 0 {
                    ctx.output.push_str(", ");
                }
                push_bind_pat(p, ctx);
            }
            ctx.output.push(')');
        }
    }
}
