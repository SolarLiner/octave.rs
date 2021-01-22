use std::sync::Arc;

use tower_lsp::{
    jsonrpc::Result as LspResult, lsp_types::*, Client, LanguageServer, LspService, Server,
};

use model::Model;
mod model;

#[derive(Debug)]
struct Backend {
    client: Client,
    model: Arc<Model>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            model: Arc::new(Model::default()),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::Full,
                )),
                completion_provider: Some(CompletionOptions::default()),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "Octave LSP".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::Info, "server initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.model.set_document(uri.clone(), text);
        let diags = self.model.get_diagnostics(&uri);
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let DidChangeTextDocumentParams {
            content_changes,
            text_document: VersionedTextDocumentIdentifier { uri, version },
        } = params;
        if let Err(err) = self.model.apply_edits(&uri, content_changes, version) {
            self.client.log_message(MessageType::Error, err).await;
        } else {
            let diags = self.model.get_diagnostics(&uri);
            self.client.publish_diagnostics(uri, diags, version).await;
        }
    }

    async fn completion(&self, _: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(
            self.model
                .get_variables()
                .into_iter()
                .map(|v| CompletionItem::new_simple(v.clone(), v))
                .collect(),
        )))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, messages) = LspService::new(Backend::new);
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;
}
