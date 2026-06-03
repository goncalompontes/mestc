use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bumpalo::Bump;
use chumsky::input::Input;
use chumsky::{Parser, input::Stream};
use lasso::Rodeo;
use logos::Logos;
use mest_core::{
    ast::{BindPat, BindPatKind, ExprKind, PatKind},
    hir::Type,
    parser::parser,
    token::Token,
    typecheck::{self, InferenceTree, Output, rename_type},
};

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use mest_core::typecheck::Input as InferInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingKind {
    Let,
    Param,
    NotBinding,
}

#[derive(Clone)]
struct AnnotationEntry {
    end: usize,
    ty: Type,
    kind: BindingKind,
}

#[derive(Clone)]
struct AnnotationMap {
    spans: BTreeMap<usize, AnnotationEntry>,
}

impl AnnotationMap {
    fn find_entry(&self, offset: usize) -> Option<(usize, usize, Type, BindingKind)> {
        self.spans
            .range(..=offset)
            .filter(|(_, e)| offset < e.end)
            .min_by_key(|(start, e)| e.end - *start)
            .map(|(start, e)| (*start, e.end, e.ty.clone(), e.kind))
    }
}

struct DocumentState {
    text: String,
    generation: u64,
    annotations: Option<AnnotationMap>,
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

fn build_annotations(source: &str) -> Option<AnnotationMap> {
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

    let result = typecheck::typecheck(&expr, &mut rodeo);

    let mut map = AnnotationMap {
        spans: BTreeMap::new(),
    };
    collect_infer_nodes(&result.tree, &mut map);
    Some(map)
}

fn collect_infer_nodes(tree: &InferenceTree, map: &mut AnnotationMap) {
    if let InferInput::Infer { expr, .. } = &tree.input {
        if let Output::Type(ty) = &tree.output {
            let range: std::ops::Range<usize> = expr.span.into_range();
            map.spans.insert(
                range.start,
                AnnotationEntry {
                    end: range.end,
                    ty: rename_type(ty),
                    kind: BindingKind::NotBinding,
                },
            );

            match &*expr.kind {
                ExprKind::Let { .. } => {}
                ExprKind::Abs { param, .. } => {
                    if let Output::Type(ty) = &tree.output {
                        if let Type::Arrow(_, _) = ty {
                            if matches!(*param.kind, PatKind::Var(_)) {
                                let renamed = rename_type(ty);
                                if let Type::Arrow(param_ty, _) = &renamed {
                                    let ident_range: std::ops::Range<usize> =
                                        param.span.into_range();
                                    map.spans.insert(
                                        ident_range.start,
                                        AnnotationEntry {
                                            end: ident_range.end,
                                            ty: (**param_ty).clone(),
                                            kind: BindingKind::Param,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for child in &tree.children {
        collect_infer_nodes(child, map);
    }
    if let InferInput::Infer { expr, .. } = &tree.input {
        if let Output::Type(_) = &tree.output {
            if let ExprKind::Let { bindings, .. } = &*expr.kind {
                for binding in bindings.iter() {
                    let value_span: std::ops::Range<usize> =
                        binding.value.span.into_range();
                    if let Some(child) = tree.children.iter().find(|c| {
                        if let InferInput::Infer { expr, .. } = &c.input {
                            let expr_range: std::ops::Range<usize> =
                                expr.span.into_range();
                            expr_range == value_span
                        } else {
                            false
                        }
                    }) {
                        if let Output::Type(val_ty) = &child.output {
                            let renamed = rename_type(val_ty);
                            collect_bind_pat_types(&binding.pat, &renamed, map);
                        }
                    }
                }
            }
        }
    }
}

fn collect_bind_pat_types(pat: &BindPat, ty: &Type, map: &mut AnnotationMap) {
    match pat.kind {
        BindPatKind::Var(name) => {
            let ident_range: std::ops::Range<usize> = name.span.into_range();
            map.spans.insert(
                ident_range.start,
                AnnotationEntry {
                    end: ident_range.end,
                    ty: ty.clone(),
                    kind: BindingKind::Let,
                },
            );
        }
        BindPatKind::Tuple(pats) => {
            if let Type::Tuple(types) = ty {
                for (sub_pat, sub_ty) in pats.iter().zip(types.iter()) {
                    collect_bind_pat_types(sub_pat, sub_ty, map);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotations_let_var_binding() {
        let source = "let x = 1 in x";
        let map = build_annotations(source).unwrap();
        let x_binding_offset = source.find("x").unwrap();
        assert!(
            map.spans.contains_key(&x_binding_offset),
            "expected annotation for binding 'x' at offset {}",
            x_binding_offset
        );
        let entry = map.spans.get(&x_binding_offset).unwrap();
        assert_eq!(entry.kind, BindingKind::Let);
        assert_eq!(entry.ty, Type::Int);
    }

    #[test]
    fn test_annotations_let_tuple_binding() {
        let source = "let (a, b) = (1, true) in b";
        let map = build_annotations(source).unwrap();
        let a_offset = source.find('a').unwrap();
        assert!(
            map.spans.contains_key(&a_offset),
            "expected annotation for binding 'a' at offset {}",
            a_offset
        );
        let a_entry = map.spans.get(&a_offset).unwrap();
        assert_eq!(a_entry.kind, BindingKind::Let);
        assert_eq!(a_entry.ty, Type::Int);
        let eq_pos = source.find('=').unwrap();
        let binding_b_offset = source[..eq_pos].rfind('b').unwrap();
        assert!(
            map.spans.contains_key(&binding_b_offset),
            "expected annotation for binding 'b' at offset {}",
            binding_b_offset
        );
        let b_entry = map.spans.get(&binding_b_offset).unwrap();
        assert_eq!(b_entry.kind, BindingKind::Let);
        assert_eq!(b_entry.ty, Type::Bool);
    }

    #[test]
    fn test_annotations_let_and_bindings() {
        let source = "let a = 1 and b = true in b";
        let map = build_annotations(source).unwrap();
        let a_offset = source.find('a').unwrap();
        assert_eq!(map.spans.get(&a_offset).unwrap().kind, BindingKind::Let);
        assert_eq!(map.spans.get(&a_offset).unwrap().ty, Type::Int);
        let eq_first = source.find('=').unwrap();
        let eq_second = source[eq_first + 1..].find('=').unwrap() + eq_first + 1;
        let b_binding_offset = source[..eq_second].rfind('b').unwrap();
        assert_eq!(
            map.spans.get(&b_binding_offset).unwrap().ty,
            Type::Bool
        );
    }
}

impl Backend {
    fn schedule_update(&self, uri: Url, generation: u64) {
        let documents = self.documents.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;

            let (text, gen_id) = {
                let docs = documents.lock().unwrap();
                match docs.get(&uri) {
                    Some(doc) => (doc.text.clone(), doc.generation),
                    None => return,
                }
            };

            if gen_id != generation {
                return;
            }

            let annotations = build_annotations(&text);
            let mut docs = documents.lock().unwrap();
            if let Some(doc) = docs.get_mut(&uri) {
                if doc.generation == generation {
                    doc.annotations = annotations;
                }
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

        let annotations = build_annotations(&text);

        let mut docs = self.documents.lock().unwrap();
        docs.insert(
            uri,
            DocumentState {
                text,
                generation: 0,
                annotations,
            },
        );
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let new_text = params.content_changes.into_iter().last().unwrap().text;

        let gen_id = {
            let mut docs = self.documents.lock().unwrap();
            let doc = docs.entry(uri.clone()).or_insert(DocumentState {
                text: String::new(),
                generation: 0,
                annotations: None,
            });
            doc.text = new_text.clone();
            doc.generation += 1;
            doc.generation
        };

        self.schedule_update(uri, gen_id);
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

        let (ty_str, kind) = {
            let docs = self.documents.lock().unwrap();
            let doc = match docs.get(&uri) {
                Some(doc) => doc,
                None => return Ok(None),
            };
            let annotations = match doc.annotations.as_ref() {
                Some(a) => a,
                None => return Ok(None),
            };
            match annotations.find_entry(offset) {
                Some((_, _, ty, kind)) => (ty.to_string(), kind),
                None => return Ok(None),
            }
        };

        let word_at = |offset: usize| -> (usize, usize) {
            let start = text[..offset]
                .rfind(|c: char| !c.is_alphanumeric() && c != '_')
                .map(|i| i + 1)
                .unwrap_or(0);
            let end = text[offset..]
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .map(|i| offset + i)
                .unwrap_or(text.len());
            (start, end)
        };

        let (word_start, word_end) = word_at(offset);
        let word = &text[word_start..word_end];

        let hover_value = match kind {
            BindingKind::Let => format!("```mest\nlet {}: {}\n```", word, ty_str),
            BindingKind::Param => format!("```mest\n{}: {}\n```", word, ty_str),
            BindingKind::NotBinding => format!("```mest\n: {}\n```", ty_str),
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover_value,
            }),
            range: Some(Range {
                start: offset_to_lsp(&text, word_start),
                end: offset_to_lsp(&text, word_end),
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

        let entries = {
            let docs = self.documents.lock().unwrap();
            let doc = match docs.get(&uri) {
                Some(doc) => doc,
                None => return Ok(None),
            };
            let annotations = match doc.annotations.as_ref() {
                Some(a) => a,
                None => return Ok(None),
            };

            let result: Vec<_> = annotations
                .spans
                .iter()
                .filter(|(_, e)| e.kind == BindingKind::Let || e.kind == BindingKind::Param)
                .filter(|(_, e)| {
                    let pos = offset_to_lsp(&doc.text, e.end);
                    pos.line >= range.start.line && pos.line <= range.end.line
                })
                .map(|(start, e)| (*start, e.end, e.ty.clone()))
                .collect();
            result
        };

        let hints: Vec<InlayHint> = entries
            .into_iter()
            .map(|(_, end, ty)| {
                let pos = offset_to_lsp(&text, end);
                InlayHint {
                    position: pos,
                    label: InlayHintLabel::String(format!(":{}", ty)),
                    kind: Some(InlayHintKind::TYPE),
                    padding_left: Some(true),
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
