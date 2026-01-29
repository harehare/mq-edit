use lsp_types::{
    ClientCapabilities, CompletionClientCapabilities, CompletionContext, CompletionParams,
    CompletionResponse, CompletionTriggerKind, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, GotoCapability, GotoDefinitionParams, GotoDefinitionResponse,
    InitializeParams, InitializeResult, InitializedParams, Location, Position,
    PublishDiagnosticsClientCapabilities, PublishDiagnosticsParams, ReferenceContext,
    ReferenceParams, SemanticTokens, SemanticTokensClientCapabilities, SemanticTokensParams,
    TextDocumentClientCapabilities, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier,
    WorkDoneProgressParams, WorkspaceFolder,
    notification::{Notification, PublishDiagnostics},
    request::{
        Completion, GotoDefinition, Initialize, References, Request, SemanticTokensFullRequest,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, path::Path};

/// JSON-RPC message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Request(RequestMessage),
    Response(ResponseMessage),
    Notification(NotificationMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMessage {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

/// Events that the LSP client can send to the application
#[derive(Debug, Clone)]
pub enum LspEvent {
    /// Diagnostics received from the server
    Diagnostics(PublishDiagnosticsParams),
    /// Semantic tokens received from the server
    SemanticTokens(String, SemanticTokens), // (file_uri, tokens)
    /// Completion items received from the server
    Completion(CompletionResponse),
    /// Definition location received from the server
    Definition(GotoDefinitionResponse),
    /// References received from the server
    References(Vec<Location>),
    /// Server initialized successfully with completion trigger characters
    Initialized(Vec<String>), // trigger_characters
    /// Error occurred
    Error(String),
}

/// LSP client for communicating with a language server
pub struct LspClient {
    /// The language server process
    process: Child,
    /// The next request ID
    next_id: Arc<AtomicU64>,
    /// Pending requests waiting for responses
    pending_requests: Arc<Mutex<HashMap<u64, String>>>,
    /// Language ID for this client
    language_id: String,
    /// Workspace root path
    root_path: PathBuf,
}

impl LspClient {
    /// Create a new LSP client
    pub fn new(
        command: &str,
        args: &[String],
        language_id: String,
        root_path: PathBuf,
    ) -> miette::Result<(Self, mpsc::Receiver<LspEvent>)> {
        // Create event channel
        let (event_tx, event_rx) = mpsc::channel();

        // Start the LSP server process
        let mut process = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| miette::miette!("Failed to start LSP server '{}': {}", command, e))?;

        // Get process stdin and stdout
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| miette::miette!("Failed to get LSP server stdout"))?;

        let pending_requests = Arc::new(Mutex::new(HashMap::new()));

        let client = Self {
            process,
            next_id: Arc::new(AtomicU64::new(1)),
            pending_requests: Arc::clone(&pending_requests),
            language_id,
            root_path,
        };

        // Start background task to read messages from the server
        let event_tx_clone = event_tx.clone();
        std::thread::spawn(move || {
            Self::read_messages_sync(stdout, event_tx_clone, pending_requests);
        });

        Ok((client, event_rx))
    }

    /// Initialize the LSP server
    pub fn initialize(&mut self) -> miette::Result<()> {
        let root_uri = Uri::from_str(&format!("file:///{}", self.root_path.display()))
            .map_err(|e| miette::miette!("Failed to convert root path to URI: {}", e))?;

        // Build client capabilities with semantic tokens support
        let capabilities = ClientCapabilities {
            text_document: Some(TextDocumentClientCapabilities {
                semantic_tokens: Some(SemanticTokensClientCapabilities {
                    dynamic_registration: Some(false),
                    requests: lsp_types::SemanticTokensClientCapabilitiesRequests {
                        full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
                        range: Some(false),
                    },
                    token_types: vec![
                        lsp_types::SemanticTokenType::NAMESPACE,
                        lsp_types::SemanticTokenType::TYPE,
                        lsp_types::SemanticTokenType::CLASS,
                        lsp_types::SemanticTokenType::ENUM,
                        lsp_types::SemanticTokenType::INTERFACE,
                        lsp_types::SemanticTokenType::STRUCT,
                        lsp_types::SemanticTokenType::TYPE_PARAMETER,
                        lsp_types::SemanticTokenType::PARAMETER,
                        lsp_types::SemanticTokenType::VARIABLE,
                        lsp_types::SemanticTokenType::PROPERTY,
                        lsp_types::SemanticTokenType::ENUM_MEMBER,
                        lsp_types::SemanticTokenType::FUNCTION,
                        lsp_types::SemanticTokenType::METHOD,
                        lsp_types::SemanticTokenType::MACRO,
                        lsp_types::SemanticTokenType::KEYWORD,
                        lsp_types::SemanticTokenType::COMMENT,
                        lsp_types::SemanticTokenType::STRING,
                        lsp_types::SemanticTokenType::NUMBER,
                        lsp_types::SemanticTokenType::OPERATOR,
                    ],
                    token_modifiers: vec![
                        lsp_types::SemanticTokenModifier::DECLARATION,
                        lsp_types::SemanticTokenModifier::DEFINITION,
                        lsp_types::SemanticTokenModifier::READONLY,
                        lsp_types::SemanticTokenModifier::STATIC,
                        lsp_types::SemanticTokenModifier::DEPRECATED,
                        lsp_types::SemanticTokenModifier::ABSTRACT,
                        lsp_types::SemanticTokenModifier::ASYNC,
                        lsp_types::SemanticTokenModifier::MODIFICATION,
                        lsp_types::SemanticTokenModifier::DOCUMENTATION,
                        lsp_types::SemanticTokenModifier::DEFAULT_LIBRARY,
                    ],
                    formats: vec![lsp_types::TokenFormat::RELATIVE],
                    overlapping_token_support: Some(false),
                    multiline_token_support: Some(false),
                    server_cancel_support: Some(false),
                    augments_syntax_tokens: Some(true),
                }),
                completion: Some(CompletionClientCapabilities {
                    dynamic_registration: Some(false),
                    completion_item: None,
                    completion_item_kind: None,
                    context_support: Some(true),
                    insert_text_mode: None,
                    completion_list: None,
                }),
                definition: Some(GotoCapability {
                    dynamic_registration: Some(false),
                    link_support: Some(false),
                }),
                references: Some(lsp_types::DynamicRegistrationClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                    related_information: Some(true),
                    tag_support: None,
                    version_support: Some(true),
                    code_description_support: Some(false),
                    data_support: Some(false),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: None,
            root_uri: Some(root_uri.clone()),
            initialization_options: None,
            capabilities,
            trace: None,
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: self
                    .root_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            }]),
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.send_request::<Initialize>(params)?;
        Ok(())
    }

    /// Send the initialized notification
    pub fn initialized(&mut self) -> miette::Result<()> {
        self.send_notification("initialized", InitializedParams {})?;
        Ok(())
    }

    /// Notify the server that a document was opened
    pub fn did_open(&mut self, file_path: &Path, content: &str) -> miette::Result<()> {
        let uri = Uri::from_str(&format!("file:///{}", file_path.display()))
            .map_err(|e| miette::miette!("Failed to convert file path to URI: {}", e))?;

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: self.language_id.clone(),
                version: 1,
                text: content.to_string(),
            },
        };

        self.send_notification("textDocument/didOpen", params)?;
        Ok(())
    }

    /// Notify the server that a document was changed
    pub fn did_change(
        &mut self,
        file_path: &Path,
        version: i32,
        content: &str,
    ) -> miette::Result<()> {
        let uri = Uri::from_str(&format!("file:///{}", file_path.display()))
            .map_err(|e| miette::miette!("Failed to convert file path to URI: {}", e))?;

        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: content.to_string(),
            }],
        };

        self.send_notification("textDocument/didChange", params)?;
        Ok(())
    }

    /// Request semantic tokens for a document
    pub fn request_semantic_tokens(&mut self, file_path: &Path) -> miette::Result<()> {
        let uri = Uri::from_str(&format!("file:///{}", file_path.display()))
            .map_err(|e| miette::miette!("Failed to convert file path to URI: {}", e))?;

        let params = SemanticTokensParams {
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
            text_document: TextDocumentIdentifier { uri },
        };

        self.send_request::<SemanticTokensFullRequest>(params)?;
        Ok(())
    }

    /// Request completion at a position
    pub fn request_completion(
        &mut self,
        file_path: &Path,
        line: u32,
        character: u32,
        trigger_character: Option<String>,
    ) -> miette::Result<()> {
        let uri = Uri::from_str(&format!("file:///{}", file_path.display()))
            .map_err(|e| miette::miette!("Failed to convert file path to URI: {}", e))?;

        let context = Some(CompletionContext {
            trigger_kind: if trigger_character.is_some() {
                CompletionTriggerKind::TRIGGER_CHARACTER
            } else {
                CompletionTriggerKind::INVOKED
            },
            trigger_character,
        });

        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
            context,
        };

        self.send_request::<Completion>(params)?;
        Ok(())
    }

    /// Request go to definition at a position
    pub fn request_definition(
        &mut self,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> miette::Result<()> {
        let uri = Uri::from_str(&format!("file:///{}", file_path.display()))
            .map_err(|e| miette::miette!("Failed to convert file path to URI: {}", e))?;

        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        };

        self.send_request::<GotoDefinition>(params)?;
        Ok(())
    }

    /// Request find references at a position
    pub fn request_references(
        &mut self,
        file_path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> miette::Result<()> {
        let uri = Uri::from_str(&format!("file:///{}", file_path.display()))
            .map_err(|e| miette::miette!("Failed to convert file path to URI: {}", e))?;

        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            context: ReferenceContext {
                include_declaration,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        };

        self.send_request::<References>(params)?;
        Ok(())
    }

    /// Send a request to the LSP server
    fn send_request<R: Request>(&mut self, params: R::Params) -> miette::Result<()>
    where
        R::Params: Serialize,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let request = RequestMessage {
            jsonrpc: "2.0".to_string(),
            id,
            method: R::METHOD.to_string(),
            params: serde_json::to_value(params)
                .map_err(|e| miette::miette!("Failed to serialize request params: {}", e))?,
        };

        // Store the pending request
        self.pending_requests
            .lock()
            .unwrap()
            .insert(id, R::METHOD.to_string());

        self.send_message(&Message::Request(request))?;
        Ok(())
    }

    /// Send a notification to the LSP server
    fn send_notification<P: Serialize>(&mut self, method: &str, params: P) -> miette::Result<()> {
        let notification = NotificationMessage {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: serde_json::to_value(params)
                .map_err(|e| miette::miette!("Failed to serialize notification params: {}", e))?,
        };

        self.send_message(&Message::Notification(notification))?;
        Ok(())
    }

    /// Send a message to the LSP server
    fn send_message(&mut self, message: &Message) -> miette::Result<()> {
        let json = serde_json::to_string(message)
            .map_err(|e| miette::miette!("Failed to serialize message: {}", e))?;

        let content = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);

        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or_else(|| miette::miette!("Failed to get LSP server stdin"))?;

        stdin
            .write_all(content.as_bytes())
            .map_err(|e| miette::miette!("Failed to write to LSP server: {}", e))?;

        stdin
            .flush()
            .map_err(|e| miette::miette!("Failed to flush LSP server stdin: {}", e))?;

        Ok(())
    }

    /// Read messages from the LSP server (runs in background thread)
    fn read_messages_sync(
        stdout: std::process::ChildStdout,
        event_tx: mpsc::Sender<LspEvent>,
        pending_requests: Arc<Mutex<HashMap<u64, String>>>,
    ) {
        let mut reader = BufReader::new(stdout);
        let mut content_length = 0;

        loop {
            // Read headers
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                break;
            }

            if line.trim().is_empty() {
                // End of headers, read content
                if content_length > 0 {
                    let mut buffer = vec![0; content_length];
                    if reader.read_exact(&mut buffer).is_err() {
                        break;
                    }

                    // Parse the JSON message
                    if let Ok(json_str) = String::from_utf8(buffer)
                        && let Ok(message) = serde_json::from_str::<Message>(&json_str)
                    {
                        Self::handle_message(message, &event_tx, &pending_requests);
                    }

                    content_length = 0;
                }
            } else if line.starts_with("Content-Length:") {
                // Parse content length
                if let Some(len_str) = line.strip_prefix("Content-Length:")
                    && let Ok(len) = len_str.trim().parse()
                {
                    content_length = len;
                }
            }
        }

        let _ = event_tx.send(LspEvent::Error("LSP server connection closed".to_string()));
    }

    /// Handle a message from the LSP server
    fn handle_message(
        message: Message,
        event_tx: &mpsc::Sender<LspEvent>,
        pending_requests: &Arc<Mutex<HashMap<u64, String>>>,
    ) {
        match message {
            Message::Response(response) => {
                // Handle response
                if let Some(error) = response.error {
                    let _ = event_tx.send(LspEvent::Error(format!(
                        "LSP error: {} (code: {})",
                        error.message, error.code
                    )));
                } else if let Some(result) = response.result {
                    // Get the method for this request
                    let method = pending_requests.lock().unwrap().remove(&response.id);

                    if let Some(method) = method {
                        // Route based on the request method
                        match method.as_str() {
                            "initialize" => {
                                // Extract trigger characters from initialize result
                                let trigger_chars = if let Ok(init_result) =
                                    serde_json::from_value::<InitializeResult>(result.clone())
                                {
                                    init_result
                                        .capabilities
                                        .completion_provider
                                        .and_then(|cp| cp.trigger_characters)
                                        .unwrap_or_default()
                                } else {
                                    Vec::new()
                                };
                                let _ = event_tx.send(LspEvent::Initialized(trigger_chars));
                            }
                            "textDocument/definition" => {
                                if let Ok(definition) =
                                    serde_json::from_value::<GotoDefinitionResponse>(result)
                                {
                                    let _ = event_tx.send(LspEvent::Definition(definition));
                                }
                            }
                            "textDocument/references" => {
                                if let Ok(references) =
                                    serde_json::from_value::<Option<Vec<Location>>>(result)
                                    && let Some(locations) = references
                                {
                                    let _ = event_tx.send(LspEvent::References(locations));
                                }
                            }
                            "textDocument/completion" => {
                                if let Ok(completion) =
                                    serde_json::from_value::<CompletionResponse>(result)
                                {
                                    let _ = event_tx.send(LspEvent::Completion(completion));
                                }
                            }
                            "textDocument/semanticTokens/full" => {
                                if let Ok(tokens) =
                                    serde_json::from_value::<Option<SemanticTokens>>(result)
                                    && let Some(tokens) = tokens
                                {
                                    let _ = event_tx
                                        .send(LspEvent::SemanticTokens(String::new(), tokens));
                                }
                            }
                            _ => {
                                // Unknown method - log or ignore
                            }
                        }
                    }
                }
            }
            Message::Notification(notification) => {
                // Handle notification
                if notification.method == PublishDiagnostics::METHOD
                    && let Ok(params) =
                        serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                {
                    let _ = event_tx.send(LspEvent::Diagnostics(params));
                }
            }
            Message::Request(_) => {
                // We don't expect requests from the server in this simple implementation
            }
        }
    }

    /// Shutdown the LSP client
    pub fn shutdown(&mut self) -> miette::Result<()> {
        let _ = self.process.kill();
        Ok(())
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let request = RequestMessage {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "initialize".to_string(),
            params: serde_json::json!({}),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_response_message() {
        let response = ResponseMessage {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!({"success": true})),
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_notification_message() {
        let notification = NotificationMessage {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: serde_json::json!({}),
        };

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("\"method\":\"textDocument/didOpen\""));
        assert!(!json.contains("\"id\""));
    }
}
