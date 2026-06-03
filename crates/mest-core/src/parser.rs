use bumpalo::Bump;
use chumsky::{
    IterParser, Parser,
    error::Rich,
    extra::{self, SimpleState},
    input::ValueInput,
    pratt::{infix, left, right},
    primitive::just,
    recursive::recursive,
    select,
    span::SimpleSpan,
};
use lasso::Rodeo;

use crate::{
    ast::{
        BinOp, BindPat, Binding, Expr, ExprKind, Ident, Literal, Pat, PatKind, TypeDef, TypeExpr,
        TypeExprKind, UnaryOp, VariantDef,
    },
    token::Token,
};

type MapExtra<'a, 'b, I, E> = chumsky::input::MapExtra<'a, 'b, I, E>;
type Extra<'tok, 'src> = extra::Full<Rich<'tok, Token<'src>>, SimpleState<Rodeo>, ()>;

type BoxedParser<'tok, 'src, 'bump, I, O> = chumsky::Boxed<'tok, 'tok, I, O, Extra<'tok, 'src>>;

fn literal_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    select! {
        Token::Int(num)   => Literal::Int(num),
        Token::Float(num) => Literal::Float(num),
        Token::True       => Literal::Bool(true),
        Token::False      => Literal::Bool(false),
    }
    .map_with(|lit, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| {
        ExprKind::literal(bump, e.span(), lit)
    })
    .boxed()
}

fn ident_parser<'tok, 'src, 'bump, I>() -> BoxedParser<'tok, 'src, 'bump, I, Ident>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    select! { Token::Ident(name) => name }
        .map_with(
            |name, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| Ident {
                name: e.state().get_or_intern(name),
                span: e.span(),
            },
        )
        .boxed()
}

fn ident_expr_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    ident_parser()
        .map_with(|name, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| {
            ExprKind::ident(bump, e.span(), name)
        })
        .boxed()
}

fn pat_atom_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
) -> BoxedParser<'tok, 'src, 'bump, I, Pat<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    recursive(|pat| {
        let wildcard = just(Token::Ident("_")).map_with(
            |_, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| Pat {
                kind: bump.alloc(PatKind::Wildcard),
                span: e.span(),
            },
        );

        let pat_lit = select! {
            Token::Int(n)   => Literal::Int(n),
            Token::Float(f) => Literal::Float(f),
            Token::True     => Literal::Bool(true),
            Token::False    => Literal::Bool(false),
        }
        .map_with(|lit, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| Pat {
            kind: bump.alloc(PatKind::Lit(lit)),
            span: e.span(),
        });

        let pat_var =
            ident_parser().map_with(|name, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| Pat {
                kind: bump.alloc(PatKind::Var(name)),
                span: e.span(),
            });

        let tuple_pat = just(Token::LParen)
            .ignore_then(
                pat.clone()
                    .then_ignore(just(Token::Comma))
                    .then_ignore(just(Token::RParen))
                    .map(|p| vec![p])
                    .or(pat
                        .clone()
                        .separated_by(just(Token::Comma))
                        .at_least(2)
                        .collect::<Vec<_>>()
                        .then_ignore(just(Token::RParen))),
            )
            .map_with(|pats, e| Pat {
                kind: bump.alloc(PatKind::Tuple(bump.alloc_slice_fill_iter(pats))),
                span: e.span(),
            });

        wildcard.or(pat_lit).or(pat_var).or(tuple_pat)
    })
    .boxed()
}

/// Pattern parser with App folding for constructor patterns like `Some x`.
/// Used in match arms and lambda parameters.
fn pat_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
) -> BoxedParser<'tok, 'src, 'bump, I, Pat<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    let atom = pat_atom_parser(bump);
    atom.clone()
        .foldl_with(atom.repeated(), |func, arg, e| {
            let func = bump.alloc(func);
            let arg = bump.alloc(arg);
            Pat {
                kind: bump.alloc(PatKind::App { func, arg }),
                span: e.span(),
            }
        })
        .boxed()
}

fn bind_pat_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
) -> BoxedParser<'tok, 'src, 'bump, I, BindPat<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    recursive(|pat| {
        let pat_var =
            ident_parser().map_with(|name, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| {
                BindPat::var(bump, name, e.span())
            });

        let tuple_pat = just(Token::LParen)
            .ignore_then(
                pat.clone()
                    .then_ignore(just(Token::Comma))
                    .then_ignore(just(Token::RParen))
                    .map(|p| vec![p])
                    .or(pat
                        .clone()
                        .separated_by(just(Token::Comma))
                        .at_least(2)
                        .collect::<Vec<_>>()
                        .then_ignore(just(Token::RParen))),
            )
            .map_with(|pats, e| BindPat::tuple(bump, pats.into_iter(), e.span()));

        pat_var.or(tuple_pat)
    })
    .boxed()
}

fn atom_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    let tuple_or_group = just(Token::LParen)
        .ignore_then(expr.clone())
        .then(
            just(Token::Comma)
                .ignore_then(
                    expr.clone()
                        .separated_by(just(Token::Comma))
                        .collect::<Vec<_>>(),
                )
                .then_ignore(just(Token::RParen))
                .or(just(Token::RParen).map(|_| vec![])),
        )
        .map_with(|(first, rest), e| {
            if rest.is_empty() {
                first
            } else {
                let mut items = vec![first];
                items.extend(rest);
                ExprKind::tuple_expr(bump, e.span(), bump.alloc_slice_fill_iter(items))
            }
        });

    literal_parser(bump)
        .or(ident_expr_parser(bump))
        .or(tuple_or_group)
        .boxed()
}

fn call_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    let atom = atom_parser(bump, expr);
    atom.clone()
        .foldl_with(atom.repeated(), |func, arg, e| {
            ExprKind::app(bump, e.span(), func, arg)
        })
        .boxed()
}

fn unary_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    let call = call_parser(bump, expr);

    let neg = just(Token::Minus).repeated().foldr_with(call, |_, rhs, e| {
        ExprKind::unaryop(bump, e.span(), UnaryOp::Neg, rhs)
    });

    just(Token::Bang)
        .repeated()
        .foldr_with(neg, |_, rhs, e| {
            ExprKind::unaryop(bump, e.span(), UnaryOp::Not, rhs)
        })
        .boxed()
}

fn pratt_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    let unary = unary_parser(bump, expr);

    unary
        .pratt((
            infix(right(4), just(Token::Caret), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Pow, lhs, rhs)
            }),
            infix(left(3), just(Token::Star), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Mul, lhs, rhs)
            }),
            infix(left(3), just(Token::Slash), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Div, lhs, rhs)
            }),
            infix(left(2), just(Token::Plus), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Add, lhs, rhs)
            }),
            infix(left(2), just(Token::Minus), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Sub, lhs, rhs)
            }),
            infix(left(1), just(Token::EqEq), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Eq, lhs, rhs)
            }),
            infix(left(1), just(Token::BangEq), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::NotEq, lhs, rhs)
            }),
            infix(left(1), just(Token::Lt), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Lt, lhs, rhs)
            }),
            infix(left(1), just(Token::Gt), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Gt, lhs, rhs)
            }),
            infix(left(1), just(Token::LtEq), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Le, lhs, rhs)
            }),
            infix(left(1), just(Token::GtEq), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Ge, lhs, rhs)
            }),
            infix(left(0), just(Token::AndAnd), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::And, lhs, rhs)
            }),
            infix(left(0), just(Token::PipePipe), |lhs, _, rhs, e| {
                ExprKind::binop(bump, e.span(), BinOp::Or, lhs, rhs)
            }),
        ))
        .boxed()
}

fn if_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    pratt_expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    just(Token::If)
        .ignore_then(pratt_expr.clone())
        .then_ignore(just(Token::Then))
        .then(pratt_expr)
        .then_ignore(just(Token::Else))
        .then(expr)
        .map_with(|((cond, then_expr), else_expr), e| {
            ExprKind::if_expr(bump, e.span(), cond, then_expr, else_expr)
        })
        .boxed()
}

fn match_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    pratt_expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    let arm = just(Token::Pipe)
        .ignore_then(pat_parser(bump))
        .then_ignore(just(Token::StrongArrow))
        .then(expr);

    just(Token::Match)
        .ignore_then(pratt_expr)
        .then(arm.repeated().at_least(1).collect::<Vec<_>>())
        .map_with(|(scrutinee, arms), e| {
            let arms = bump.alloc_slice_fill_iter(arms.into_iter());
            ExprKind::match_expr(bump, e.span(), scrutinee, arms)
        })
        .boxed()
}

fn type_expr_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
) -> BoxedParser<'tok, 'src, 'bump, I, TypeExpr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    recursive(|type_expr| {
        let type_atom = {
            let builtin = select! {
                Token::Ident("Int")   => "Int",
                Token::Ident("Float") => "Float",
                Token::Ident("Bool")  => "Bool",
            }
            .map_with(
                |name, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| TypeExpr {
                    kind: bump.alloc(TypeExprKind::Builtin(name)),
                    span: e.span(),
                },
            );

            let ident_type = select! { Token::Ident(name) => name }.map_with(
                |name, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| {
                    let ident = Ident {
                        name: e.state().get_or_intern(name),
                        span: e.span(),
                    };
                    TypeExpr {
                        kind: bump.alloc(if ident_uses_uppercase(name) {
                            TypeExprKind::Named(ident)
                        } else {
                            TypeExprKind::Var(ident)
                        }),
                        span: e.span(),
                    }
                },
            );

            let grouped = just(Token::LParen)
                .ignore_then(type_expr.clone())
                .then_ignore(just(Token::RParen))
                .or(just(Token::LParen)
                    .ignore_then(
                        type_expr
                            .clone()
                            .separated_by(just(Token::Comma))
                            .at_least(2)
                            .collect::<Vec<_>>(),
                    )
                    .then_ignore(just(Token::RParen))
                    .map_with(|types, e| TypeExpr {
                        kind: bump.alloc(TypeExprKind::Tuple(bump.alloc_slice_fill_iter(types))),
                        span: e.span(),
                    }));

            builtin.or(ident_type).or(grouped)
        };

        // Right-associative arrow
        type_atom
            .clone()
            .then(
                just(Token::StrongArrow)
                    .ignore_then(type_expr.clone())
                    .or_not(),
            )
            .map_with(|(from, to), e| match to {
                Some(to) => TypeExpr {
                    kind: bump.alloc(TypeExprKind::Arrow {
                        from: bump.alloc(from),
                        to: bump.alloc(to),
                    }),
                    span: e.span(),
                },
                None => from,
            })
    })
    .boxed()
}

fn type_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    let type_expr = type_expr_parser(bump);

    let variant = ident_parser()
        .then(type_expr.clone().or_not())
        .map_with(|(name, arg), _e| VariantDef { name, arg });

    let union_def = just(Token::Pipe)
        .ignore_then(variant.clone())
        .then(
            just(Token::Pipe)
                .ignore_then(variant)
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map_with(|(first, rest), _| {
            let mut variants = vec![first];
            variants.extend(rest);
            variants
        });

    // RHS: either a union (starting with |) or a type expression (named tuple sugar)
    let rhs = union_def
        .map(|variants| (variants, None))
        .or(type_expr.clone().map(|te| (vec![], Some(te))));

    // type <name> = <union>  or  type <name> = <type_expr>
    let type_def = ident_parser()
        .then_ignore(just(Token::Eq))
        .then(rhs)
        .map_with(|(name, (mut variants, named_tuple_te)), _e| {
            if let Some(te) = named_tuple_te {
                // Named tuple sugar: constructor name = type name
                variants.push(VariantDef {
                    name,
                    arg: Some(te),
                });
            }
            TypeDef {
                name,
                params: bump.alloc_slice_fill_iter([]),
                variants: bump.alloc_slice_fill_iter(variants),
            }
        });

    just(Token::Type)
        .ignore_then(type_def.clone())
        .then(
            just(Token::And)
                .ignore_then(type_def)
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(Token::In))
        .then(expr)
        .map_with(|((first, mut extra), body), e| {
            let defs = bump.alloc_slice_fill_iter({
                extra.insert(0, first);
                extra
            });
            ExprKind::type_expr(bump, e.span(), defs, body)
        })
        .boxed()
}

use lasso::Spur;
use std::collections::{HashMap, HashSet};

/// Collect all constructor names from `type ... in` definitions in the AST.
pub fn collect_constructors<'bump>(expr: &Expr<'bump>) -> HashMap<Spur, Vec<VariantDef<'bump>>> {
    let mut map = HashMap::new();
    collect_constructors_in_expr(expr, &mut map);
    map
}

fn collect_constructors_in_expr<'bump>(
    expr: &Expr<'bump>,
    map: &mut HashMap<Spur, Vec<VariantDef<'bump>>>,
) {
    match &**expr {
        ExprKind::Type { definitions, body } => {
            for def in definitions.iter() {
                map.insert(def.name.name, def.variants.to_vec());
            }
            collect_constructors_in_expr(body, map);
        }
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_constructors_in_expr(cond, map);
            collect_constructors_in_expr(then_expr, map);
            collect_constructors_in_expr(else_expr, map);
        }
        ExprKind::Let { bindings, body } => {
            for binding in bindings.iter() {
                collect_constructors_in_expr(&binding.value, map);
            }
            collect_constructors_in_expr(body, map);
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_constructors_in_expr(scrutinee, map);
            for (_, body) in arms.iter() {
                collect_constructors_in_expr(body, map);
            }
        }
        ExprKind::Abs { body, .. } => {
            collect_constructors_in_expr(body, map);
        }
        ExprKind::App { func, arg } => {
            collect_constructors_in_expr(func, map);
            collect_constructors_in_expr(arg, map);
        }
        ExprKind::Tuple(items) => {
            for item in items.iter() {
                collect_constructors_in_expr(item, map);
            }
        }
        ExprKind::Constructor { arg, .. } => {
            if let Some(arg) = arg {
                collect_constructors_in_expr(arg, map);
            }
        }
        ExprKind::Literal(_)
        | ExprKind::Var(_)
        | ExprKind::BinOp { .. }
        | ExprKind::UnaryOp { .. } => {}
    }
}

/// Build a set of constructor names from the collected map.
pub fn constructor_names(map: &HashMap<Spur, Vec<VariantDef>>) -> HashSet<Spur> {
    let mut names = HashSet::new();
    for (_type_name, variants) in map {
        for v in variants {
            names.insert(v.name.name);
        }
    }
    names
}

/// Resolve `Var(name)` → `Constructor` and `App(Constructor, arg)` → `Constructor(name, Some(arg))`
/// in expressions, where `name` is in the constructor set.
pub fn resolve_constructors<'bump>(
    expr: &Expr<'bump>,
    constructors: &HashSet<Spur>,
    bump: &'bump Bump,
) -> Expr<'bump> {
    resolve_expr(expr, constructors, bump)
}

fn resolve_expr<'bump>(
    expr: &Expr<'bump>,
    constructors: &HashSet<Spur>,
    bump: &'bump Bump,
) -> Expr<'bump> {
    match &**expr {
        ExprKind::Var(ident) if constructors.contains(&ident.name) => {
            ExprKind::constructor_expr(bump, expr.span, *ident, None)
        }
        ExprKind::App { func, arg } => {
            let func = resolve_expr(func, constructors, bump);
            let arg = resolve_expr(arg, constructors, bump);
            // Check if the function is now a nullary constructor
            if let ExprKind::Constructor { name, arg: None } = *func.kind {
                ExprKind::constructor_expr(bump, expr.span, name, Some(arg))
            } else {
                ExprKind::app(bump, expr.span, func, arg)
            }
        }
        ExprKind::Type { definitions, body } => {
            let body = resolve_expr(body, constructors, bump);
            ExprKind::type_expr(bump, expr.span, definitions, body)
        }
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => {
            let cond = resolve_expr(cond, constructors, bump);
            let then_expr = resolve_expr(then_expr, constructors, bump);
            let else_expr = resolve_expr(else_expr, constructors, bump);
            ExprKind::if_expr(bump, expr.span, cond, then_expr, else_expr)
        }
        ExprKind::Let { bindings, body } => {
            let new_bindings = bump.alloc_slice_fill_iter(bindings.iter().map(|b| {
                Binding::new(
                    bump,
                    b.rec,
                    b.pat,
                    resolve_expr(&b.value, constructors, bump),
                )
            }));
            let body = resolve_expr(body, constructors, bump);
            ExprKind::let_expr(bump, expr.span, new_bindings, body)
        }
        ExprKind::Match { scrutinee, arms } => {
            let scrutinee = resolve_expr(scrutinee, constructors, bump);
            let new_arms = bump.alloc_slice_fill_iter(arms.iter().map(|(pat, body)| {
                let pat = resolve_pat(pat, constructors, bump);
                let body = resolve_expr(body, constructors, bump);
                (pat, body)
            }));
            ExprKind::match_expr(bump, expr.span, scrutinee, new_arms)
        }
        ExprKind::Abs { param, body } => {
            let param = resolve_pat(param, constructors, bump);
            let body = resolve_expr(body, constructors, bump);
            ExprKind::lambda(bump, expr.span, param, body)
        }
        ExprKind::Tuple(items) => {
            let new_items = bump.alloc_slice_fill_iter(
                items
                    .iter()
                    .map(|item| resolve_expr(item, constructors, bump)),
            );
            ExprKind::tuple_expr(bump, expr.span, new_items)
        }
        ExprKind::Constructor { name, arg } => {
            let new_arg = match arg {
                Some(a) => Some(resolve_expr(a, constructors, bump)),
                None => None,
            };
            ExprKind::constructor_expr(bump, expr.span, *name, new_arg)
        }
        ExprKind::Var(_)
        | ExprKind::Literal(_)
        | ExprKind::BinOp { .. }
        | ExprKind::UnaryOp { .. } => *expr,
    }
}

/// Resolve pattern `Var(name)` → `Constructor` and `App(Var(name), arg)` → `Constructor(name, Some(arg))`.
pub fn resolve_pat<'bump>(
    pat: &Pat<'bump>,
    constructors: &HashSet<Spur>,
    bump: &'bump Bump,
) -> Pat<'bump> {
    match &**pat {
        PatKind::Var(ident) if constructors.contains(&ident.name) => Pat {
            kind: bump.alloc(PatKind::Constructor {
                name: *ident,
                arg: None,
            }),
            span: pat.span,
        },
        PatKind::App { func, arg } => {
            let func = resolve_pat(func, constructors, bump);
            let arg = resolve_pat(arg, constructors, bump);
            if let PatKind::Constructor { name, arg: None } = &*func {
                Pat {
                    kind: bump.alloc(PatKind::Constructor {
                        name: *name,
                        arg: Some(bump.alloc(arg)),
                    }),
                    span: pat.span,
                }
            } else {
                Pat {
                    kind: bump.alloc(PatKind::App {
                        func: bump.alloc(func),
                        arg: bump.alloc(arg),
                    }),
                    span: pat.span,
                }
            }
        }
        _ => {
            // Walk children for other pattern types
            match &**pat {
                PatKind::Or(a, b) => {
                    let a = bump.alloc(resolve_pat(a, constructors, bump));
                    let b = bump.alloc(resolve_pat(b, constructors, bump));
                    Pat {
                        kind: bump.alloc(PatKind::Or(a, b)),
                        span: pat.span,
                    }
                }
                PatKind::Tuple(pats) => {
                    let new_pats = bump.alloc_slice_fill_iter(
                        pats.iter().map(|p| resolve_pat(p, constructors, bump)),
                    );
                    Pat {
                        kind: bump.alloc(PatKind::Tuple(new_pats)),
                        span: pat.span,
                    }
                }
                PatKind::Constructor { name, arg } => {
                    let new_arg = arg.map(|a| {
                        let p: &'bump Pat<'bump> = bump.alloc(resolve_pat(a, constructors, bump));
                        p
                    });
                    Pat {
                        kind: bump.alloc(PatKind::Constructor {
                            name: *name,
                            arg: new_arg,
                        }),
                        span: pat.span,
                    }
                }
                PatKind::Wildcard | PatKind::Var(_) | PatKind::Lit(_) => *pat,
                PatKind::App { .. } => unreachable!(), // handled above
            }
        }
    }
}

fn ident_uses_uppercase(name: &str) -> bool {
    name.starts_with(|c: char| c.is_uppercase())
}

fn let_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    // this is just a binding: <rec>? <pattern> (<and> <pattern>)* = <expr>
    let binding = just(Token::Rec)
        .or_not()
        .then(bind_pat_parser(bump))
        .then(pat_atom_parser(bump).repeated().collect::<Vec<_>>())
        .then_ignore(just(Token::Eq))
        .then(expr.clone())
        .map_with(|(((rec, name), params), value), _| {
            let value = params.into_iter().rev().fold(value, |body, param| {
                ExprKind::lambda(bump, param.span, param, body)
            });
            Binding::new(bump, rec.is_some(), name, value)
        });

    // this is: <let> <binding> ( <and> <binding> )* <in>
    just(Token::Let)
        .ignore_then(binding.clone())
        .then(
            just(Token::And)
                .ignore_then(binding)
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(Token::In))
        .then(expr)
        .map_with(|((first_binding, mut extra_bindings), body), e| {
            let bindings = bump.alloc_slice_fill_iter({
                extra_bindings.insert(0, first_binding);
                extra_bindings
            }) as &_;
            ExprKind::let_expr(bump, e.span(), bindings, body)
        })
        .boxed()
}

fn lambda_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    just(Token::Pipe)
        .ignore_then(pat_parser(bump))
        .then_ignore(just(Token::Pipe))
        .then(expr)
        .map_with(|(param, body), e| ExprKind::lambda(bump, e.span(), param, body))
        .boxed()
}

pub fn parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
) -> impl Parser<'tok, I, Expr<'bump>, Extra<'tok, 'src>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    recursive(|expr| {
        let expr_boxed = expr.clone().boxed();
        let pratt = pratt_parser(bump, expr_boxed.clone());

        type_parser(bump, expr_boxed.clone())
            .or(if_parser(bump, pratt.clone(), expr_boxed.clone()))
            .or(match_parser(bump, pratt.clone(), expr_boxed.clone()))
            .or(let_parser(bump, expr_boxed.clone()))
            .or(lambda_parser(bump, expr_boxed))
            .or(pratt)
    })
}
