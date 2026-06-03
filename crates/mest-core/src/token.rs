use logos::Logos;
use std::fmt::Display;

#[derive(Debug, Clone, Logos)]
pub enum Token<'a> {
    #[token("if")]
    If,
    #[token("then")]
    Then,
    #[token("else")]
    Else,
    #[token("rec")]
    Rec,
    #[token("let")]
    Let,
    #[token("in")]
    In,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("match")]
    Match,
    #[token("and")]
    And,
    #[token("type")]
    Type,

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice())]
    Ident(&'a str),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().unwrap())]
    Float(f64),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().unwrap())]
    Int(i64),

    // comparison
    #[token("==")]
    EqEq,
    #[token("!=")]
    BangEq,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,

    // logical
    #[token("&&")]
    AndAnd,
    #[token("||")]
    PipePipe,
    #[token("!")]
    Bang,

    // arithmetic
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("^")]
    Caret,

    // punctuation
    #[token("|")]
    Pipe,
    #[token("=")]
    Eq,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token(",")]
    Comma,
    #[token("=>")]
    StrongArrow,

    #[regex(r"[ \t\r\n]+", logos::skip)]
    WhiteSpace,

    Error,
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::If => write!(f, "if"),
            Token::Then => write!(f, "then"),
            Token::Else => write!(f, "else"),
            Token::Rec => write!(f, "rec"),
            Token::Let => write!(f, "let"),
            Token::In => write!(f, "in"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Ident(v) => write!(f, "{v}"),
            Token::Float(n) => write!(f, "{n}"),
            Token::Int(n) => write!(f, "{n}"),
            Token::EqEq => write!(f, "=="),
            Token::BangEq => write!(f, "!="),
            Token::LtEq => write!(f, "<="),
            Token::GtEq => write!(f, ">="),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::AndAnd => write!(f, "&&"),
            Token::PipePipe => write!(f, "||"),
            Token::Bang => write!(f, "!"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Caret => write!(f, "^"),
            Token::Pipe => write!(f, "|"),
            Token::Eq => write!(f, "="),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::WhiteSpace => write!(f, "<whitespace>"),
            Token::Error => write!(f, "<error>"),
            Token::Match => write!(f, "match"),
            Token::And => write!(f, "and"),
            Token::Comma => write!(f, ","),
            Token::StrongArrow => write!(f, "=>"),
            Token::Type => write!(f, "type"),
        }
    }
}

impl PartialEq for Token<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ident(l), Self::Ident(r)) => l == r,
            (Self::Float(l), Self::Float(r)) => (l.is_nan() && r.is_nan()) || l == r,
            (Self::Int(l), Self::Int(r)) => l == r,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl Eq for Token<'_> {}
