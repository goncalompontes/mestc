use ariadne::{Color, Label, Report, Source};
use bumpalo::Bump;
use chumsky::{
    Parser,
    input::{Input, Stream},
};
use clap::{Parser as ClapParser, Subcommand};
use lasso::Rodeo;
use logos::Logos;
use mest_core::{
    ast::Env,
    parser::parser,
    token::Token,
    typecheck,
    visitor::AstPrinter,
};

#[derive(ClapParser)]
#[command(name = "mylang", about = "A simple functional language")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Evaluate an expression from a string
    Eval {
        /// The expression to evaluate
        expr: String,
    },
    /// Evaluate an expression from a file
    Run {
        /// Path to the source file
        path: std::path::PathBuf,
    },
    /// Lex a source string and print the token stream
    Lex {
        /// The expression to lex
        expr: String,
    },
    /// Parse a file and print the AST
    Parse {
        /// Path to the source file
        path: std::path::PathBuf,
    },
    /// Type-check a file and print the inferred type
    Check {
        /// Path to the source file
        path: std::path::PathBuf,
    },
}

fn run(src: &str) {
    let token_iter = Token::lexer(src).spanned().map(|(tok, span)| match tok {
        Ok(tok) => (tok, span.into()),
        Err(()) => (Token::Error, span.into()),
    });

    let token_stream =
        Stream::from_iter(token_iter).map((0..src.len()).into(), |(t, s): (_, _)| (t, s));

    let bump = Bump::new();
    let mut rodeo = chumsky::extra::SimpleState(Rodeo::new());

    match parser(&bump)
        .parse_with_state(token_stream, &mut rodeo)
        .into_result()
    {
        Err(errs) => {
            for err in errs {
                Report::build(ariadne::ReportKind::Error, ((), err.span().into_range()))
                    .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                    .with_code(3)
                    .with_message(err.to_string())
                    .with_label(
                        Label::new(((), err.span().into_range()))
                            .with_message(err.reason().to_string())
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint(Source::from(src))
                    .unwrap();
            }
        }
        Ok(expr) => {
            let mut env = Env::new();
            match expr.eval_lazy(&mut env, &rodeo) {
                Ok(res) => println!("{res}"),
                Err(err) => eprintln!("eval error: {:?}", err),
            }
        }
    }
}

fn lex(src: &str) {
    for (tok, span) in Token::lexer(src).spanned() {
        match tok {
            Ok(tok) => println!("{:?}  ({:?})", tok, span),
            Err(()) => eprintln!(
                "error at {:?}: unexpected character `{}`",
                span.clone(),
                &src[span]
            ),
        }
    }
}

fn parse(src: &str) -> Result<(), ()> {
    let token_iter = Token::lexer(src).spanned().map(|(tok, span)| match tok {
        Ok(tok) => (tok, span.into()),
        Err(()) => (Token::Error, span.into()),
    });

    let token_stream =
        Stream::from_iter(token_iter).map((0..src.len()).into(), |(t, s): (_, _)| (t, s));

    let bump = Bump::new();
    let mut rodeo = chumsky::extra::SimpleState(Rodeo::new());

    match parser(&bump)
        .parse_with_state(token_stream, &mut rodeo)
        .into_result()
    {
        Err(errs) => {
            for err in errs {
                Report::build(ariadne::ReportKind::Error, ((), err.span().into_range()))
                    .with_config(
                        ariadne::Config::new().with_index_type(ariadne::IndexType::Byte),
                    )
                    .with_code(3)
                    .with_message(err.to_string())
                    .with_label(
                        Label::new(((), err.span().into_range()))
                            .with_message(err.reason().to_string())
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint(Source::from(src))
                    .unwrap();
            }
            Err(())
        }
        Ok(expr) => {
            println!("{}", AstPrinter::print_expr(&expr, &rodeo));
            Ok(())
        }
    }
}

fn check(src: &str) -> Result<(), ()> {
    let token_iter = Token::lexer(src).spanned().map(|(tok, span)| match tok {
        Ok(tok) => (tok, span.into()),
        Err(()) => (Token::Error, span.into()),
    });

    let token_stream =
        Stream::from_iter(token_iter).map((0..src.len()).into(), |(t, s): (_, _)| (t, s));

    let bump = Bump::new();
    let mut rodeo = chumsky::extra::SimpleState(Rodeo::new());

    match parser(&bump)
        .parse_with_state(token_stream, &mut rodeo)
        .into_result()
    {
        Err(errs) => {
            for err in errs {
                Report::build(ariadne::ReportKind::Error, ((), err.span().into_range()))
                    .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                    .with_code(3)
                    .with_message(err.to_string())
                    .with_label(
                        Label::new(((), err.span().into_range()))
                            .with_message(err.reason().to_string())
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint(Source::from(src))
                    .unwrap();
            }
            Err(())
        }
        Ok(expr) => {
            let result = typecheck::typecheck(&expr, &mut rodeo);
            println!("{} : {}", AstPrinter::print_expr(&expr, &rodeo), result.ty);
            println!();
            println!("{}", result.tree.display_tree());

            if result.errors.is_empty() {
                Ok(())
            } else {
                for err in &result.errors {
                    err.to_report()
                        .eprint(ariadne::Source::from(src))
                        .unwrap();
                }
                Err(())
            }
        },
    }
}

fn main() -> Result<(), ()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Eval { expr } => {
            run(&expr);
            Ok(())
        }
        Command::Run { path } => match std::fs::read_to_string(&path) {
            Ok(src) => {
                run(&src);
                Ok(())
            }
            Err(err) => {
                eprintln!("error reading {}: {}", path.display(), err);
                Err(())
            }
        },
        Command::Lex { expr } => {
            lex(&expr);
            Ok(())
        }
        Command::Parse { path } => match std::fs::read_to_string(&path) {
            Ok(src) => parse(&src),
            Err(err) => {
                eprintln!("error reading {}: {}", path.display(), err);
                Err(())
            }
        },
        Command::Check { path } => match std::fs::read_to_string(&path) {
            Ok(src) => check(&src),
            Err(err) => {
                eprintln!("error reading {}: {}", path.display(), err);
                Err(())
            }
        },
    }
}
