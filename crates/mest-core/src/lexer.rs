use std::ops::Range;

use crate::token::Token;
use logos::Logos;

#[derive(Debug)]
pub struct LexError {
    pub span: Range<usize>,
    pub message: Option<String>,
}

pub fn tokenize(src: &str) -> (Vec<(Token, Range<usize>)>, Vec<LexError>) {
    let mut tokens = vec![];
    let mut errors = vec![];

    for (result, span) in Token::lexer(src).spanned() {
        match result {
            Ok(tok) => tokens.push((tok, span)),
            Err(_) => errors.push(LexError {
                message: Some(format!("unexpected character `{}`", &src[span.clone()])),
                span,
            }),
        }
    }

    (tokens, errors)
}
