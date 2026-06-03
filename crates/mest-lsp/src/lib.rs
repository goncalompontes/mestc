use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bumpalo::Bump;
use chumsky::input::Input;
use chumsky::{Parser, input::Stream};
use lasso::Rodeo;
use logos::Logos;
use mest_core::{
    hir::type_to_string,
    parser::{self, parser},
    token::Token,
    typecheck::{self, TypeEntryKind, TypeIndex},
};

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

mod backend;

struct DocumentState {
    text: String,
    typed: Option<(TypeIndex, Rodeo)>,
}

pub struct Backend {
    client: Client,
    documents: Arc<Mutex<HashMap<Url, DocumentState>>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn lsp_to_offset(text: &str, pos: Position) -> Option<usize> {
    let mut offset = 0usize;
    for _ in 0..pos.line {
        offset = text[offset..].find('\n')? + offset + 1;
    }
    Some(offset + pos.character as usize)
}

fn offset_to_lsp(text: &str, offset: usize) -> Position {
    let line = text[..offset].matches('\n').count() as u32;
    let col_start = text[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = (offset - col_start) as u32;
    Position::new(line, character)
}

fn line_to_byte_offset(text: &str, line: u32) -> usize {
    text.split('\n')
        .take(line as usize)
        .map(|s| s.len() + 1)
        .sum::<usize>()
        .min(text.len())
}

fn is_operator(tok: &Token) -> bool {
    use Token::*;
    matches!(
        tok,
        Plus | Minus
            | Star
            | Slash
            | Caret
            | EqEq
            | BangEq
            | Lt
            | Gt
            | LtEq
            | GtEq
            | AndAnd
            | PipePipe
            | Bang
    )
}

fn operator_type(op: &Token) -> Option<String> {
    use Token::*;
    let sig = match op {
        Plus | Minus | Star | Slash | Caret => "Int -> Int -> Int",
        EqEq | BangEq | Lt | Gt | LtEq | GtEq => "Int -> Int -> Bool",
        AndAnd | PipePipe => "Bool -> Bool -> Bool",
        Bang => "Bool -> Bool",
        _ => return None,
    };
    Some(format!("```mest\n: {}\n```", sig))
}

fn build_type_index(source: &str) -> Option<(TypeIndex, Rodeo)> {
    let token_iter = Token::lexer(source).spanned().map(|(tok, span)| match tok {
        Ok(tok) => (tok, span.into()),
        Err(()) => (Token::Error, span.into()),
    });

    let token_stream =
        Stream::from_iter(token_iter).map((0..source.len()).into(), |(t, s): (_, _)| (t, s));

    let bump = Bump::new();
    let mut rodeo = chumsky::extra::SimpleState(Rodeo::new());

    let expr = match parser(&bump)
        .parse_with_state(token_stream, &mut rodeo)
        .into_result()
    {
        Ok(expr) => expr,
        Err(_) => return None,
    };

    let constructors = parser::collect_constructors(&expr);
    let names = parser::constructor_names(&constructors);
    let bump2 = Bump::new();
    let expr = parser::resolve_constructors(&expr, &names, &bump2);

    let result = typecheck::typecheck(&expr, &mut rodeo);
    let rodeo = rodeo.0;
    Some((result.type_index, rodeo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mest_core::hir::Type;

    #[test]
    fn test_type_index_let_var_binding() {
        let source = "let x = 1 in x";
        let (ti, _) = build_type_index(source).unwrap();
        let x_binding_offset = source.find("x").unwrap();
        let entry = ti.node_at(x_binding_offset).unwrap();
        assert_eq!(entry.kind, TypeEntryKind::LetBinding);
        assert_eq!(entry.ty, Type::Int);
    }

    #[test]
    fn test_type_index_let_tuple_binding() {
        let source = "let (a, b) = (1, true) in b";
        let (ti, _) = build_type_index(source).unwrap();
        let a_offset = source.find('a').unwrap();
        let a_entry = ti.node_at(a_offset).unwrap();
        assert_eq!(a_entry.kind, TypeEntryKind::LetBinding);
        assert_eq!(a_entry.ty, Type::Int);
        let eq_pos = source.find('=').unwrap();
        let binding_b_offset = source[..eq_pos].rfind('b').unwrap();
        let b_entry = ti.node_at(binding_b_offset).unwrap();
        assert_eq!(b_entry.kind, TypeEntryKind::LetBinding);
        assert_eq!(b_entry.ty, Type::Bool);
    }

    #[test]
    fn test_type_index_let_and_bindings() {
        let source = "let a = 1 and b = true in b";
        let (ti, _) = build_type_index(source).unwrap();
        let a_offset = source.find('a').unwrap();
        assert_eq!(
            ti.node_at(a_offset).unwrap().kind,
            TypeEntryKind::LetBinding
        );
        assert_eq!(ti.node_at(a_offset).unwrap().ty, Type::Int);
        let eq_first = source.find('=').unwrap();
        let eq_second = source[eq_first + 1..].find('=').unwrap() + eq_first + 1;
        let b_binding_offset = source[..eq_second].rfind('b').unwrap();
        assert_eq!(ti.node_at(b_binding_offset).unwrap().ty, Type::Bool);
    }

    #[test]
    fn test_type_index_swap_tuple_pattern() {
        let source = "let swap = |(a, b)| (b, a) in swap";
        let (ti, _) = build_type_index(source).unwrap();
        let pipe = source.find('|').unwrap();
        let a_in_pattern = source[pipe..].find('a').unwrap() + pipe;
        let b_in_pattern = source[pipe..].find('b').unwrap() + pipe;
        let a_entry = ti.node_at(a_in_pattern).unwrap();
        let b_entry = ti.node_at(b_in_pattern).unwrap();
        assert_eq!(a_entry.kind, TypeEntryKind::ParamBinding);
        assert_eq!(b_entry.kind, TypeEntryKind::ParamBinding);
        assert_ne!(
            a_entry.ty, b_entry.ty,
            "a and b should have different types after renaming"
        );
        match (&a_entry.ty, &b_entry.ty) {
            (Type::Var(av), Type::Var(bv)) => {
                assert_ne!(av, bv, "a and b should be different type variables")
            }
            _ => panic!("expected Var types for a and b"),
        }
    }

    #[test]
    fn test_type_index_scope_independent_renaming() {
        let source = "let const = |x| |y| x in let id = |x| x in id const";
        let (ti, _) = build_type_index(source).unwrap();
        let id_binding_offset = source.find("id").unwrap();
        let id_entry = ti.node_at(id_binding_offset).unwrap();
        assert_eq!(id_entry.kind, TypeEntryKind::LetBinding);
        let id_display = id_entry.ty.to_string();
        assert_eq!(
            id_display, "(τ) -> τ",
            "id should show (τ) -> τ, got: {id_display}"
        );
    }
}

impl Backend {
    fn delayed_typecheck(&self, uri: Url) {
        let documents = self.documents.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;

            let text = {
                let docs = documents.lock().unwrap();
                docs.get(&uri).map(|d| d.text.clone())
            };
            let Some(text) = text else { return };

            let typed = build_type_index(&text);
            {
                let mut docs = documents.lock().unwrap();
                if let Some(doc) = docs.get_mut(&uri) {
                    doc.typed = typed;
                }
            }

            if let Err(e) = client
                .send_request::<tower_lsp::lsp_types::request::InlayHintRefreshRequest>(())
                .await
            {
                client
                    .log_message(MessageType::WARNING, format!("refresh failed: {e}"))
                    .await;
            }
        });
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        let type_index = build_type_index(&text);

        {
            let mut docs = self.documents.lock().unwrap();
            docs.insert(
                uri.clone(),
                DocumentState {
                    text,
                    typed: type_index,
                },
            );
        }

        if let Err(e) = self
            .client
            .send_request::<tower_lsp::lsp_types::request::InlayHintRefreshRequest>(())
            .await
        {
            self.client
                .log_message(MessageType::WARNING, format!("refresh failed: {e}"))
                .await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let new_text = params.content_changes.into_iter().last().unwrap().text;

        {
            let mut docs = self.documents.lock().unwrap();
            let doc = docs.entry(uri.clone()).or_insert(DocumentState {
                text: String::new(),
                typed: None,
            });
            doc.text = new_text;
        }

        self.delayed_typecheck(uri);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .lock()
            .unwrap()
            .remove(&params.text_document.uri);
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let text = {
            let docs = self.documents.lock().unwrap();
            match docs.get(&uri) {
                Some(doc) => doc.text.clone(),
                None => return Ok(None),
            }
        };

        let offset = match lsp_to_offset(&text, pos) {
            Some(o) => o,
            None => return Ok(None),
        };

        // Find the exact token at cursor via lexer
        let lexer = Token::lexer(&text);
        let (tok, span) = match lexer
            .spanned()
            .filter_map(|(tok, span)| match tok {
                Ok(tok) => Some((tok, span.into())),
                Err(()) => None,
            })
            .find(|(_, s): &(Token, std::ops::Range<usize>)| s.contains(&offset))
        {
            Some(v) => v,
            None => return Ok(None),
        };
        let (start, end) = (span.start, span.end);

        // Derive hover info from the token and AST/typechecker
        let hover_value = match &tok {
            Token::Ident(name) if *name == "_" => "```mest\n_ : wildcard pattern\n```".into(),
            Token::Ident(_) => {
                // Variable / constructor name — use TypeIndex
                let info = {
                    let docs = self.documents.lock().unwrap();
                    docs.get(&uri).and_then(|doc| {
                        let (type_index, rodeo) = doc.typed.as_ref()?;
                        let e = type_index.node_at(offset)?;
                        let ty_str = type_to_string(&e.ty, rodeo);
                        Some((e.start, e.end, ty_str, e.kind))
                    })
                };
                match info {
                    Some((s, e, ty_str, kind)) => {
                        let word = &text[s..e];
                        match kind {
                            TypeEntryKind::LetBinding => {
                                format!("```mest\nlet {}: {}\n```", word, ty_str)
                            }
                            TypeEntryKind::ParamBinding | TypeEntryKind::MatchBinding => {
                                format!("```mest\n{}: {}\n```", word, ty_str)
                            }
                            TypeEntryKind::Expr => format!("```mest\n: {}\n```", ty_str),
                        }
                    }
                    None => return Ok(None),
                }
            }
            Token::Int(_) | Token::Float(_) | Token::True | Token::False => {
                // Literal — find the Literal expr's type via TypeIndex
                let ty_str = {
                    let docs = self.documents.lock().unwrap();
                    docs.get(&uri).and_then(|doc| {
                        let (type_index, rodeo) = doc.typed.as_ref()?;
                        let e = type_index.node_at(offset)?;
                        Some(type_to_string(&e.ty, rodeo))
                    })
                };
                match ty_str {
                    Some(s) => format!("```mest\n: {}\n```", s),
                    None => return Ok(None),
                }
            }
            Token::Let | Token::If | Token::Match | Token::Type => {
                // Keyword with enclosing expression — show its type
                let ty_str = {
                    let docs = self.documents.lock().unwrap();
                    docs.get(&uri).and_then(|doc| {
                        let (type_index, rodeo) = doc.typed.as_ref()?;
                        let e = type_index.node_at(offset)?;
                        Some(type_to_string(&e.ty, rodeo))
                    })
                };
                match ty_str {
                    Some(s) => format!("```mest\n: {}\n```", s),
                    None => return Ok(None),
                }
            }
            tok if is_operator(tok) => match operator_type(tok) {
                Some(s) => s,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover_value,
            }),
            range: Some(Range {
                start: offset_to_lsp(&text, start),
                end: offset_to_lsp(&text, end),
            }),
        }))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let range = params.range;

        let text = {
            let docs = self.documents.lock().unwrap();
            match docs.get(&uri) {
                Some(doc) => doc.text.clone(),
                None => return Ok(None),
            }
        };

        let byte_start = line_to_byte_offset(&text, range.start.line);
        let byte_end = line_to_byte_offset(&text, range.end.line + 1).max(byte_start);

        let entries: Vec<(usize, String)> = {
            let docs = self.documents.lock().unwrap();
            let doc = match docs.get(&uri) {
                Some(doc) => doc,
                None => return Ok(None),
            };
            let (type_index, rodeo) = match doc.typed.as_ref() {
                Some(v) => v,
                None => return Ok(None),
            };
            type_index
                .entries_in_range(byte_start..byte_end)
                .into_iter()
                .map(|e| (e.end, type_to_string(&e.ty, rodeo)))
                .collect()
        };

        let hints: Vec<InlayHint> = entries
            .into_iter()
            .map(|(end, ty_str)| {
                let pos = offset_to_lsp(&text, end);
                InlayHint {
                    position: pos,
                    label: InlayHintLabel::String(format!(": {}", ty_str)),
                    kind: Some(InlayHintKind::TYPE),
                    padding_left: Some(false),
                    padding_right: None,
                    text_edits: None,
                    tooltip: None,
                    data: None,
                }
            })
            .collect();

        Ok(Some(hints))
    }
}
