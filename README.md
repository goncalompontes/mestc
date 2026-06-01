# mest — Mest compiler

A simple functional programming language for learning purposes, built with Rust.

## Install

```sh
cargo install mestc
```

## Usage

```sh
# Evaluate an expression
mest eval "let add = |x| |y| x + y in add 3 4"

# Run a .mest file
mest run program.mest

# Tokenize and print tokens
mest lex "let x = 42 in x"
```

## Build from source

```sh
cargo build --release
```

## Project structure

| Crate | Description |
|---|---|
| `mest-core` | Core language: lexer, parser, AST, type inference, evaluator |
| `mestc` | CLI binary (`mest`) |
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
