use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bumpalo::Bump;
use chumsky::input::Input;
use chumsky::{Parser, input::Stream};
use lasso::Rodeo;
use logos::Logos;
use mest_core::{
    ast::ExprKind,
    hir::Type,
    parser::parser,
    token::Token,
    typecheck::{self, rename_type, InferenceTree, Output},
};

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use mest_core::typecheck::Input as InferInput;

struct AnnotationEntry {
    end: usize,
    ty: Type,
    is_binding: bool,
}

struct AnnotationMap {
    spans: BTreeMap<usize, AnnotationEntry>,
}

impl AnnotationMap {
    fn find_entry(&self, offset: usize) -> Option<(usize, usize, Type, bool)> {
        self.spans
            .range(..=offset)
            .filter(|(_, e)| offset < e.end)
            .min_by_key(|(start, e)| e.end - *start)
            .map(|(start, e)| (*start, e.end, e.ty.clone(), e.is_binding))
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
            map.spans.insert(range.start, AnnotationEntry { end: range.end, ty: rename_type(ty), is_binding: false });

            match &*expr.kind {
                ExprKind::Let { name, .. } => {
                    if let Some(first_child) = tree.children.first() {
                        if let Output::Type(val_ty) = &first_child.output {
                            let ident_range: std::ops::Range<usize> = name.span.into_range();
                            map.spans.insert(ident_range.start, AnnotationEntry { end: ident_range.end, ty: rename_type(val_ty), is_binding: true });
                        }
                    }
                }
                ExprKind::Abs { param, .. } => {
                    if let Output::Type(ty) = &tree.output {
                        if let Type::Arrow(_, _) = ty {
                            let renamed = rename_type(ty);
                            if let Type::Arrow(param_ty, _) = &renamed {
                                let ident_range: std::ops::Range<usize> = param.span.into_range();
                                map.spans.insert(ident_range.start, AnnotationEntry { end: ident_range.end, ty: (**param_ty).clone(), is_binding: true });
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

        let ty_str = {
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
                Some((_, _, ty, _)) => ty.to_string(),
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

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```mest\n: {}\n```", ty_str),
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
                .filter(|(_, e)| e.is_binding)
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
                    label: InlayHintLabel::String(format!(": {}", ty)),
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
