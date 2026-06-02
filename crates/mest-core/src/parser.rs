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
    ast::{BinOp, ExprKind, Expr, Ident, Literal, Pat, PatKind, UnaryOp},
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
        .map_with(|name, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| {
            Ident(e.state().get_or_intern(name))
        })
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

fn pat_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
) -> BoxedParser<'tok, 'src, 'bump, I, Pat<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    let wildcard = just(Token::Ident("_"))
        .map_with(|_, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| Pat {
            kind: bump.alloc(PatKind::Wildcard),
            span: e.span(),
        });

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

    let pat_var = ident_parser()
        .map_with(|name, e: &mut MapExtra<'_, '_, I, Extra<'tok, 'src>>| Pat {
            kind: bump.alloc(PatKind::Var(name)),
            span: e.span(),
        });

    wildcard.or(pat_lit).or(pat_var).boxed()
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
    let group = expr.delimited_by(just(Token::LParen), just(Token::RParen));

    literal_parser(bump)
        .or(ident_expr_parser(bump))
        .or(group)
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

fn let_parser<'tok, 'src, 'bump, I>(
    bump: &'bump Bump,
    expr: BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>,
) -> BoxedParser<'tok, 'src, 'bump, I, Expr<'bump>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
    'src: 'tok,
    'bump: 'tok,
{
    just(Token::Let)
        .then(just(Token::Rec).or_not())
        .then(ident_parser())
        .then(ident_parser().repeated().collect::<Vec<_>>())
        .then_ignore(just(Token::Eq))
        .then(expr.clone())
        .then_ignore(just(Token::In))
        .then(expr)
        .map_with(|(((((_, rec), name), params), value), body), e| {
            let value = params.into_iter().rev().fold(value, |body, param| {
                ExprKind::lambda(bump, e.span(), param, body)
            });
            ExprKind::let_expr(bump, e.span(), name, value, body, rec.is_some())
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
        .ignore_then(ident_parser())
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

        if_parser(bump, pratt.clone(), expr_boxed.clone())
            .or(match_parser(bump, pratt.clone(), expr_boxed.clone()))
            .or(let_parser(bump, expr_boxed.clone()))
            .or(lambda_parser(bump, expr_boxed))
            .or(pratt)
    })
}
