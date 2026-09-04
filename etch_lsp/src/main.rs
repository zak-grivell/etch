use highlighting::{HighlightKind, highlight};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tokio::sync::RwLock;
use tower_lsp::{Client, LanguageServer, LspService, Server, jsonrpc::Result, lsp_types::*};

const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::COMMENT,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::STRING,
    SemanticTokenType::NUMBER,
    SemanticTokenType::new("boolean"),
    SemanticTokenType::TYPE,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::OPERATOR,
];

struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: TOKEN_TYPES.to_vec(),
                                token_modifiers: vec![],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: Some(false),
                            work_done_progress_options: Default::default(),
                        },
                    ),
                ),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into(), ":".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "Etch language server".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Etch language server initialized")
            .await;
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.update(params.text_document.uri, params.text_document.text)
            .await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.update(params.text_document.uri, change.text).await;
        }
    }
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .write()
            .await
            .remove(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let documents = self.documents.read().await;
        let Some(source) = documents.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let mut previous = Position::new(0, 0);
        let data = highlight(source)
            .into_iter()
            .filter_map(|item| {
                let start = position(source, item.span.start);
                let end = position(source, item.span.end);
                if start.line != end.line {
                    return None;
                }
                let delta_line = start.line - previous.line;
                let delta_start = if delta_line == 0 {
                    start.character - previous.character
                } else {
                    start.character
                };
                previous = start;
                Some(SemanticToken {
                    delta_line,
                    delta_start,
                    length: end.character - start.character,
                    token_type: kind_index(item.kind),
                    token_modifiers_bitset: 0,
                })
            })
            .collect();
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let documents = self.documents.read().await;
        let Some(source) = documents.get(&params.text_document_position.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(CompletionResponse::Array(completions(source))))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let documents = self.documents.read().await;
        let position_params = params.text_document_position_params;
        let Some(source) = documents.get(&position_params.text_document.uri) else {
            return Ok(None);
        };
        let Some(byte) = byte_offset(source, position_params.position) else {
            return Ok(None);
        };
        let Some(token) = highlight(source)
            .into_iter()
            .find(|token| token.span.start <= byte && byte <= token.span.end)
        else {
            return Ok(None);
        };
        let word = &source[token.span.clone()];
        let Some(contents) = hover_text(word, token.kind) else {
            return Ok(None);
        };
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: contents,
            }),
            range: Some(Range::new(
                position(source, token.span.start),
                position(source, token.span.end),
            )),
        }))
    }
}

impl Backend {
    async fn update(&self, uri: Url, source: String) {
        let diagnostics = parser::compile(&source)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|error| {
                let diagnostic = error.diagnostic();
                Diagnostic {
                    range: Range::new(
                        position(&source, diagnostic.span.start),
                        position(&source, diagnostic.span.end),
                    ),
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some(format!("etch {}", diagnostic.stage)),
                    message: diagnostic.message,
                    ..Default::default()
                }
            })
            .collect();
        self.documents.write().await.insert(uri.clone(), source);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

fn position(source: &str, byte: usize) -> Position {
    let byte = byte.min(source.len());
    let prefix = &source[..source.floor_char_boundary(byte)];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let current = prefix.rsplit_once('\n').map_or(prefix, |(_, line)| line);
    Position::new(line, current.encode_utf16().count() as u32)
}

fn kind_index(kind: HighlightKind) -> u32 {
    kind as u32
}

fn completions(source: &str) -> Vec<CompletionItem> {
    let mut items = BTreeMap::new();
    for (label, kind, detail) in [
        ("let", CompletionItemKind::KEYWORD, "Define a value"),
        ("type", CompletionItemKind::KEYWORD, "Define a type"),
        ("return", CompletionItemKind::KEYWORD, "Return from a block"),
        (
            "match",
            CompletionItemKind::KEYWORD,
            "Pattern-match a value",
        ),
        ("if", CompletionItemKind::KEYWORD, "Add a match guard"),
        ("from", CompletionItemKind::KEYWORD, "Start an import"),
        (
            "import",
            CompletionItemKind::KEYWORD,
            "Import selected names",
        ),
        ("export", CompletionItemKind::KEYWORD, "Export a definition"),
        ("true", CompletionItemKind::VALUE, "Boolean literal"),
        ("false", CompletionItemKind::VALUE, "Boolean literal"),
    ] {
        items.insert(label.to_owned(), completion_item(label, kind, detail));
    }
    for ty in ["Number", "String", "Bool", "Node", "None", "Never"] {
        items.insert(
            ty.to_owned(),
            completion_item(ty, CompletionItemKind::CLASS, "Built-in Etch type"),
        );
    }
    for token in highlight(source) {
        if matches!(token.kind, HighlightKind::Identifier | HighlightKind::Type) {
            let label = &source[token.span];
            let kind = if token.kind == HighlightKind::Type {
                CompletionItemKind::CLASS
            } else {
                CompletionItemKind::VARIABLE
            };
            items
                .entry(label.to_owned())
                .or_insert_with(|| completion_item(label, kind, "Symbol in this document"));
        }
    }
    items.into_values().collect()
}

fn completion_item(label: &str, kind: CompletionItemKind, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.into(),
        kind: Some(kind),
        detail: Some(detail.into()),
        ..Default::default()
    }
}

fn hover_text(word: &str, kind: HighlightKind) -> Option<String> {
    let documentation = match word {
        "let" => "Defines a value by matching a pattern against an expression.",
        "type" => "Defines a named Etch type.",
        "return" => "Returns a value from the current block.",
        "match" => "Evaluates pattern and optional guard arms in order.",
        "if" => "Introduces a condition on a match arm.",
        "from" => "Introduces the source path of an import.",
        "import" => "Imports names selected by a pattern.",
        "export" => "Makes a top-level definition available to importing files.",
        "Number" => {
            "The numeric type. It may carry a unit symbol, for example `Number(symbol: \"V\")`."
        }
        "String" => "The UTF-8 string type.",
        "Bool" => "The Boolean type.",
        "Node" => "A circuit connection node.",
        "None" => "The type of no value.",
        "Never" => "The type of an expression that does not produce a value.",
        "true" | "false" => "A Boolean literal.",
        _ if kind == HighlightKind::Number => {
            "A numeric literal, optionally followed by an SI prefix and unit."
        }
        _ if matches!(kind, HighlightKind::Identifier | HighlightKind::Type) => {
            "A symbol in the current Etch document."
        }
        _ => return None,
    };
    Some(format!("```etch\n{word}\n```\n\n{documentation}"))
}

fn byte_offset(source: &str, target: Position) -> Option<usize> {
    let line_start = if target.line == 0 {
        0
    } else {
        source.match_indices('\n').nth(target.line as usize - 1)?.0 + 1
    };
    let line = source[line_start..]
        .split_once('\n')
        .map_or(&source[line_start..], |(line, _)| line);
    let mut utf16 = 0u32;
    for (offset, ch) in line.char_indices() {
        if utf16 >= target.character {
            return Some(line_start + offset);
        }
        utf16 += ch.len_utf16() as u32;
    }
    (utf16 == target.character).then_some(line_start + line.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_builtins_and_document_symbols() {
        let items = completions("type Voltage = Number; let supply = 5V");
        assert!(items.iter().any(|item| item.label == "match"));
        assert!(items.iter().any(|item| item.label == "Voltage"));
        assert!(items.iter().any(|item| item.label == "supply"));
    }

    #[test]
    fn converts_utf16_editor_positions_to_bytes() {
        assert_eq!(byte_offset("🔌value", Position::new(0, 2)), Some(4));
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Arc::default(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
