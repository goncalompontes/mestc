# mestc — Mest compiler

A simple functional programming language for learning purposes, built with Rust.

## Features

- First-class functions, closures, currying
- Lazy evaluation with memoized thunks
- Pattern matching
- Recursive bindings (`let rec`)
- Lambda expressions (`|param| body`)
- Standard arithmetic, comparison, and logical operators
- Hindley-Milner type inference (in progress)

## Building

```sh
cargo build --release
```

## Usage

```sh
# Evaluate an expression
mestc eval "let add = |x| |y| x + y in add 3 4"

# Run a .mest file
mestc run program.mest

# Tokenize and print tokens
mestc lex "let x = 42 in x"
```

## Project structure

| Crate | Description |
|---|---|
| `mest-core` | Core language: lexer, parser, AST, type inference, evaluator |
| `mest-cli` | CLI binary (`mestc`) |
| `mest-lsp` | LSP server for editor integration |

## Language example

```
let rec factorial n =
    match n
    | 0 => 1
    | n => n * factorial (n - 1)
in

let compose = |f| |g| |x| f (g x) in

factorial 10
```
