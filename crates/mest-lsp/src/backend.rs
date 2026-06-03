use std::sync::Arc;

use lasso::Rodeo;
use logos::Lexer;
use mest_core::{token::Token, typecheck::TypeIndex};
use tokio::sync::Mutex;
use tower_lsp::{Client, LanguageServer, async_trait, lsp_types::{InitializeParams, InitializeResult}};

struct DocState<'src> {
    tokens: Lexer<'src, Token<'src>>,
    typed: Option<(TypeIndex, Rodeo)>,
}

struct DocStore<'src> {
    store: Arc<Mutex<DocState<'src>>>,
}

struct Backend<'src> {
    client: Client,
    documents: DocStore<'src>,
}

#[async_trait]
impl<'src> LanguageServer for Backend<'src> {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        
    }
}