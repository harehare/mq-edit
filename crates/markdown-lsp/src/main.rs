use std::path::PathBuf;
use std::sync::mpsc;

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    CompletionOptions, InitializeParams, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind,
    notification::{DidChangeTextDocument, DidOpenTextDocument, Notification as _},
    request::{Completion, GotoDefinition, References, Request as _},
};

use markdown_lsp::backend::LspBackend;
use markdown_lsp::client::LspEvent;
use markdown_lsp::markdown_lsp::MarkdownLsp;

fn main() -> miette::Result<()> {
    eprintln!("markdown-lsp: starting...");

    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![
                "#".to_string(),
                "[".to_string(),
                "!".to_string(),
                "`".to_string(),
                "-".to_string(),
            ]),
            ..Default::default()
        }),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        references_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    })
    .unwrap();

    let init_params = match connection.initialize(server_capabilities) {
        Ok(params) => params,
        Err(e) => {
            if e.channel_is_disconnected() {
                io_threads.join().map_err(|e| miette::miette!("{e:?}"))?;
            }
            return Err(miette::miette!("Initialize failed: {e}"));
        }
    };

    let init_params: InitializeParams = serde_json::from_value(init_params)
        .map_err(|e| miette::miette!("Failed to parse InitializeParams: {e}"))?;

    #[allow(deprecated)]
    let root_path = init_params
        .root_uri
        .map(|uri| uri_to_path(&uri))
        .unwrap_or_else(|| PathBuf::from("."));

    eprintln!("markdown-lsp: initialized with root {:?}", root_path);

    main_loop(&connection, root_path)?;

    io_threads.join().map_err(|e| miette::miette!("{e:?}"))?;
    eprintln!("markdown-lsp: shutdown complete");
    Ok(())
}

fn main_loop(connection: &Connection, root_path: PathBuf) -> miette::Result<()> {
    let (event_tx, event_rx) = mpsc::channel();
    let mut lsp = MarkdownLsp::new(root_path, event_tx);

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req).map_err(|e| miette::miette!("{e}"))? {
                    return Ok(());
                }
                handle_request(&mut lsp, &event_rx, connection, req)?;
            }
            Message::Notification(notif) => {
                handle_notification(&mut lsp, connection, &event_rx, notif)?;
            }
            Message::Response(_) => {}
        }
    }

    Ok(())
}

fn handle_request(
    lsp: &mut MarkdownLsp,
    event_rx: &mpsc::Receiver<LspEvent>,
    connection: &Connection,
    req: Request,
) -> miette::Result<()> {
    match req.method.as_str() {
        Completion::METHOD => {
            let (id, params) = extract_request::<Completion>(req)?;
            let pos = params.text_document_position;
            let file_path = uri_to_path(&pos.text_document.uri);

            lsp.request_completion(
                &file_path,
                pos.position.line,
                pos.position.character,
                params.context.and_then(|c| c.trigger_character),
            )
            .ok();

            let result = match event_rx.try_recv() {
                Ok(LspEvent::Completion(resp)) => serde_json::to_value(resp).ok(),
                _ => Some(serde_json::Value::Null),
            };

            send_response(connection, id, result);
        }
        GotoDefinition::METHOD => {
            let (id, params) = extract_request::<GotoDefinition>(req)?;
            let pos = params.text_document_position_params;
            let file_path = uri_to_path(&pos.text_document.uri);

            lsp.request_definition(&file_path, pos.position.line, pos.position.character)
                .ok();

            let result = match event_rx.try_recv() {
                Ok(LspEvent::Definition(resp)) => serde_json::to_value(resp).ok(),
                _ => Some(serde_json::Value::Null),
            };

            send_response(connection, id, result);
        }
        References::METHOD => {
            let (id, params) = extract_request::<References>(req)?;
            let pos = params.text_document_position;
            let file_path = uri_to_path(&pos.text_document.uri);

            lsp.request_references(
                &file_path,
                pos.position.line,
                pos.position.character,
                params.context.include_declaration,
            )
            .ok();

            let result = match event_rx.try_recv() {
                Ok(LspEvent::References(resp)) => serde_json::to_value(resp).ok(),
                _ => Some(serde_json::Value::Null),
            };

            send_response(connection, id, result);
        }
        _ => {
            eprintln!("markdown-lsp: unhandled request: {}", req.method);
        }
    }

    Ok(())
}

fn handle_notification(
    lsp: &mut MarkdownLsp,
    connection: &Connection,
    event_rx: &mpsc::Receiver<LspEvent>,
    notif: Notification,
) -> miette::Result<()> {
    match notif.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params: lsp_types::DidOpenTextDocumentParams =
                serde_json::from_value(notif.params)
                    .map_err(|e| miette::miette!("Failed to parse didOpen params: {e}"))?;
            let file_path = uri_to_path(&params.text_document.uri);

            lsp.did_open(&file_path, &params.text_document.text).ok();

            drain_diagnostics(event_rx, connection);
        }
        DidChangeTextDocument::METHOD => {
            let params: lsp_types::DidChangeTextDocumentParams =
                serde_json::from_value(notif.params)
                    .map_err(|e| miette::miette!("Failed to parse didChange params: {e}"))?;
            let file_path = uri_to_path(&params.text_document.uri);

            if let Some(change) = params.content_changes.into_iter().last() {
                lsp.did_change(&file_path, params.text_document.version, &change.text)
                    .ok();
            }

            drain_diagnostics(event_rx, connection);
        }
        _ => {}
    }

    Ok(())
}

fn drain_diagnostics(event_rx: &mpsc::Receiver<LspEvent>, connection: &Connection) {
    while let Ok(event) = event_rx.try_recv() {
        if let LspEvent::Diagnostics(params) = event {
            let notif = Notification::new(
                lsp_types::notification::PublishDiagnostics::METHOD.to_string(),
                params,
            );
            connection.sender.send(Message::Notification(notif)).ok();
        }
    }
}

fn extract_request<R: lsp_types::request::Request>(
    req: Request,
) -> miette::Result<(RequestId, R::Params)>
where
    R::Params: serde::de::DeserializeOwned,
{
    let (id, params) = req
        .extract::<R::Params>(R::METHOD)
        .map_err(|e| miette::miette!("Failed to extract request: {e:?}"))?;
    Ok((id, params))
}

fn send_response(connection: &Connection, id: RequestId, result: Option<serde_json::Value>) {
    let resp = Response {
        id,
        result,
        error: None,
    };
    connection.sender.send(Message::Response(resp)).ok();
}

fn uri_to_path(uri: &lsp_types::Uri) -> PathBuf {
    let s = uri.as_str();
    PathBuf::from(s.strip_prefix("file://").unwrap_or(s))
}
