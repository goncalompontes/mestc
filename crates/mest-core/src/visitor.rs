use crate::ast::{BinOp, Expr, ExprKind, Literal, Pat, PatKind, UnaryOp};
use lasso::Rodeo;

pub trait Visitor<'bump, Ctx>: Sized {
    fn visit_expr(&mut self, _expr: &'bump Expr<'bump>, _ctx: &mut Ctx) {}

    fn visit_pat(&mut self, _pat: &'bump Pat<'bump>, _ctx: &mut Ctx) {}

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
            ExprKind::Let {
                name: _,
                value,
                body,
                rec: _,
            } => {
                self.walk_expr(value, ctx);
                self.walk_expr(body, ctx);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.walk_expr(scrutinee, ctx);
                for (pat, body) in arms.iter() {
                    self.walk_pat(pat, ctx);
                    self.walk_expr(body, ctx);
                }
            }
            ExprKind::Abs { param: _, body } => {
                self.walk_expr(body, ctx);
            }
            ExprKind::App { func, arg } => {
                self.walk_expr(func, ctx);
                self.walk_expr(arg, ctx);
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
    pub fn print_expr<'bump, 'a>(expr: &'bump Expr<'bump>, rodeo: &'a Rodeo) -> DisplayExpr<'a, 'bump> {
        DisplayExpr { expr, rodeo }
    }
}

impl<'bump> Expr<'bump> {
    pub fn display<'a>(&'bump self, rodeo: &'a Rodeo) -> DisplayExpr<'a, 'bump> {
        DisplayExpr { expr: self, rodeo }
    }
}

fn indent(level: usize) -> String {
    "  ".repeat(level)
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
                ctx.output.push_str(ctx.rodeo.resolve(&ident.0));
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
            ExprKind::Let {
                name,
                value,
                body,
                rec,
            } => {
                if *rec {
                    ctx.output.push_str("let rec ");
                } else {
                    ctx.output.push_str("let ");
                }
                ctx.output.push_str(ctx.rodeo.resolve(&name.0));
                ctx.output.push_str(" = ");
                ctx.indent += 1;
                self.visit_expr(value, ctx);
                ctx.output.push('\n');
                ctx.indent -= 1;
                ctx.output.push_str(&indent(ctx.indent));
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
                ctx.output.push_str(ctx.rodeo.resolve(&param.0));
                ctx.output.push_str("| ");
                self.visit_expr(body, ctx);
            }
            ExprKind::App { func, arg } => {
                parexpr(self, func, ctx, true);
                ctx.output.push(' ');
                parexpr(self, arg, ctx, true);
            }
        }
    }

    fn visit_pat(&mut self, pat: &'bump Pat<'bump>, ctx: &mut PrintCtx<'_>) {
        match &**pat {
            PatKind::Wildcard => ctx.output.push('_'),
            PatKind::Var(ident) => ctx.output.push_str(ctx.rodeo.resolve(&ident.0)),
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
        }
    }
}
