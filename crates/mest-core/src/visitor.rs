use crate::ast::{BinOp, Expr, ExprKind, Literal, Pat, UnaryOp};
use lasso::Rodeo;

pub trait Visitor<'bump, Ctx>: Sized {
    fn visit_expr(&mut self, _expr: &'bump Expr<'bump>, _ctx: &mut Ctx) {}

    fn visit_pat(&mut self, _pat: &'bump Pat<'bump>, _ctx: &mut Ctx) {}

    fn walk_expr(&mut self, expr: &'bump Expr<'bump>, ctx: &mut Ctx) {
        self.visit_expr(expr, ctx);
        match &*expr.kind {
            ExprKind::Literal(_) | ExprKind::Ident(_) => {}
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
        match pat {
            Pat::Wildcard | Pat::Var(_) | Pat::Lit(_) => {}
            Pat::Or(a, b) => {
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
}

impl<'a> PrintCtx<'a> {
    pub fn new(rodeo: &'a Rodeo) -> Self {
        Self { rodeo, indent: 0 }
    }
}

pub struct AstPrinter;

impl AstPrinter {
    pub fn print_expr<'bump>(expr: &'bump Expr<'bump>, ctx: &mut PrintCtx) {
        AstPrinter.visit_expr(expr, ctx);
    }
}

impl<'bump> Visitor<'bump, PrintCtx<'_>> for AstPrinter {
    fn visit_expr(&mut self, expr: &'bump Expr<'bump>, ctx: &mut PrintCtx<'_>) {
        let indent = "  ".repeat(ctx.indent);
        match &*expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(n) => println!("{indent}Int({n})"),
                Literal::Float(f) => println!("{indent}Float({f})"),
                Literal::Bool(b) => println!("{indent}Bool({b})"),
            },
            ExprKind::Ident(ident) => {
                println!("{indent}Ident({})", ctx.rodeo.resolve(&ident.0));
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                println!("{indent}If");
                ctx.indent += 1;
                self.visit_expr(cond, ctx);
                println!("{}Then", "  ".repeat(ctx.indent - 1));
                self.visit_expr(then_expr, ctx);
                println!("{}Else", "  ".repeat(ctx.indent - 1));
                self.visit_expr(else_expr, ctx);
                ctx.indent -= 1;
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                let op_str = match op {
                    BinOp::Eq => "==",
                    BinOp::NotEq => "!=",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::Le => "<=",
                    BinOp::Ge => ">=",
                    BinOp::And => "&&",
                    BinOp::Or => "||",
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Pow => "^",
                };
                println!("{indent}BinOp({op_str})");
                ctx.indent += 1;
                self.visit_expr(lhs, ctx);
                self.visit_expr(rhs, ctx);
                ctx.indent -= 1;
            }
            ExprKind::UnaryOp { op, rhs } => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                println!("{indent}UnaryOp({op_str})");
                ctx.indent += 1;
                self.visit_expr(rhs, ctx);
                ctx.indent -= 1;
            }
            ExprKind::Let {
                name,
                value,
                body,
                rec,
            } => {
                if *rec {
                    println!("{indent}LetRec({})", ctx.rodeo.resolve(&name.0));
                } else {
                    println!("{indent}Let({})", ctx.rodeo.resolve(&name.0));
                }
                ctx.indent += 1;
                self.visit_expr(value, ctx);
                self.visit_expr(body, ctx);
                ctx.indent -= 1;
            }
            ExprKind::Match { scrutinee, arms } => {
                println!("{indent}Match");
                ctx.indent += 1;
                self.visit_expr(scrutinee, ctx);
                for (pat, body) in arms.iter() {
                    self.visit_pat(pat, ctx);
                    self.visit_expr(body, ctx);
                }
                ctx.indent -= 1;
            }
            ExprKind::Abs { param, body } => {
                println!("{indent}Lambda({})", ctx.rodeo.resolve(&param.0));
                ctx.indent += 1;
                self.visit_expr(body, ctx);
                ctx.indent -= 1;
            }
            ExprKind::App { func, arg } => {
                println!("{indent}App");
                ctx.indent += 1;
                self.visit_expr(func, ctx);
                self.visit_expr(arg, ctx);
                ctx.indent -= 1;
            }
        }
    }

    fn visit_pat(&mut self, pat: &'bump Pat<'bump>, ctx: &mut PrintCtx<'_>) {
        let indent = "  ".repeat(ctx.indent);
        match pat {
            Pat::Wildcard => println!("{indent}Wildcard"),
            Pat::Var(ident) => {
                println!("{indent}Var({})", ctx.rodeo.resolve(&ident.0));
            }
            Pat::Lit(lit) => match lit {
                Literal::Int(n) => println!("{indent}Lit(Int({n}))"),
                Literal::Float(f) => println!("{indent}Lit(Float({f}))"),
                Literal::Bool(b) => println!("{indent}Lit(Bool({b}))"),
            },
            Pat::Or(a, b) => {
                println!("{indent}Or");
                ctx.indent += 1;
                self.visit_pat(a, ctx);
                self.visit_pat(b, ctx);
                ctx.indent -= 1;
            }
        }
    }
}
