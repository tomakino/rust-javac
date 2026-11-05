
//! Minimal LSP server for the analysis API.
//!
//! Depends only on javac-analysis-api and lsp-types; the backend is injected
//! as a dyn AnalysisHost. Protocol plumbing is a small stdio JSON-RPC loop.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use javac_analysis_api::{AnalysisHost, Diagnostic, Position, Range, Severity, SymbolKind};
use lsp_types::Url;
use serde_json::{json, Value};

pub fn run(
    host: &mut dyn AnalysisHost,
    mut stdin: impl BufRead,
    mut stdout: impl Write,
) {
    let mut docs: HashMap<PathBuf, String> = HashMap::new();
    loop {
        let Some(message) = read_message(&mut stdin) else { break };
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let id = message.get("id").cloned();
        match method {
            "initialize" => respond(&mut stdout, id, json!({ "capabilities": capabilities() })),
            "initialized" => {}
            "shutdown" => respond(&mut stdout, id, Value::Null),
            "exit" => break,
            "textDocument/didOpen" => {
                if let Some((uri, text)) = document_text(&message, "text") {
                    if let Ok(path) = uri_to_path(&uri) {
                        docs.insert(path.clone(), text.clone());
                        publish_diagnostics(&mut stdout, host, &uri, &path, &text);
                    }
                }
            }
            "textDocument/didChange" => {
                if let Some((uri, text)) = change_text(&message) {
                    if let Ok(path) = uri_to_path(&uri) {
                        docs.insert(path.clone(), text.clone());
                        publish_diagnostics(&mut stdout, host, &uri, &path, &text);
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = document_uri(&message) {
                    if let Ok(path) = uri_to_path(&uri) {
                        docs.remove(&path);
                    }
                    publish_empty(&mut stdout, &uri);
                }
            }
            "textDocument/didSave" => {
                if let Some(uri) = document_uri(&message) {
                    if let Ok(path) = uri_to_path(&uri) {
                        let text = message
                            .get("params")
                            .and_then(|params| params.get("text"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or_else(|| docs.get(&path).cloned());
                        if let Some(text) = text {
                            docs.insert(path.clone(), text.clone());
                            publish_diagnostics(&mut stdout, host, &uri, &path, &text);
                        }
                    }
                }
            }
            "$/cancelRequest" => {}
            "textDocument/completion" => {
                respond(&mut stdout, id, completion(host, &message));
            }
            "textDocument/definition" => {
                respond(&mut stdout, id, definition(host, &message));
            }
            "textDocument/hover" => {
                respond(&mut stdout, id, hover(host, &message));
            }
            "textDocument/documentSymbol" => {
                respond(&mut stdout, id, document_symbols(host, &message));
            }
            "textDocument/references" => {
                respond(&mut stdout, id, references(host, &message));
            }
            _ => {
                if let Some(id) = id {
                    respond_error(&mut stdout, id, -32601, "method not found");
                }
            }
        }
    }
}

fn capabilities() -> Value {
    json!({
        "textDocumentSync": { "openClose": true, "change": 1, "save": { "includeText": true } },
        "completionProvider": { "triggerCharacters": [".", ":"] },
        "definitionProvider": true,
        "hoverProvider": true,
        "documentSymbolProvider": true,
        "referencesProvider": true
    })
}

fn document_text(message: &Value, key: &str) -> Option<(String, String)> {
    let params = message.get("params")?;
    let uri = params.get("textDocument")?.get("uri")?.as_str()?.to_string();
    let text = params
        .get("textDocument")?
        .get("text")?
        .as_str()?
        .to_string();
    let _ = key;
    Some((uri, text))
}

fn change_text(message: &Value) -> Option<(String, String)> {
    let params = message.get("params")?;
    let uri = params.get("textDocument")?.get("uri")?.as_str()?.to_string();
    let text = params
        .get("contentChanges")?
        .get(0)?
        .get("text")?
        .as_str()?
        .to_string();
    Some((uri, text))
}

fn document_uri(message: &Value) -> Option<String> {
    let params = message.get("params")?;
    params
        .get("textDocument")?
        .get("uri")?
        .as_str()
        .map(str::to_string)
}

fn publish_empty(stdout: &mut impl Write, uri: &str) {
    notify(
        stdout,
        "textDocument/publishDiagnostics",
        json!({ "uri": uri, "diagnostics": [] }),
    );
}

fn uri_to_path(uri: &str) -> Result<PathBuf, ()> {
    let url = Url::parse(uri).map_err(|_| ())?;
    url.to_file_path().map_err(|_| ())
}

fn publish_diagnostics(
    stdout: &mut impl Write,
    host: &mut dyn AnalysisHost,
    uri: &str,
    path: &PathBuf,
    text: &str,
) {
    let diagnostics = host.analyze(path, Some(text));
    let payload: Vec<Value> = diagnostics.iter().map(diagnostic_json).collect();
    notify(
        stdout,
        "textDocument/publishDiagnostics",
        json!({ "uri": uri, "diagnostics": payload }),
    );
}

fn diagnostic_json(diagnostic: &Diagnostic) -> Value {
    json!({
        "range": range_json(diagnostic.range),
        "severity": severity_number(diagnostic.severity),
        "source": "ravac",
        "message": diagnostic.message,
        "code": diagnostic.code,
    })
}

fn severity_number(severity: Severity) -> i32 {
    match severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Information => 3,
        Severity::Hint => 4,
    }
}

fn range_json(range: Range) -> Value {
    json!({
        "start": { "line": range.start.line, "character": range.start.column },
        "end": { "line": range.end.line, "character": range.end.column },
    })
}

fn completion(host: &mut dyn AnalysisHost, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    let items: Vec<Value> = host
        .completion(&path, position)
        .into_iter()
        .map(|item| {
            json!({
                "label": item.label,
                "kind": symbol_kind_number(item.kind),
                "detail": item.detail,
            })
        })
        .collect();
    Value::Array(items)
}

fn definition(host: &mut dyn AnalysisHost, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    match host.definition(&path, position) {
        Some(location) => {
            let uri = Url::from_file_path(&location.file)
                .map(|url| url.to_string())
                .unwrap_or_default();
            json!({ "uri": uri, "range": range_json(location.range) })
        }
        None => Value::Null,
    }
}

fn references(host: &mut dyn AnalysisHost, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    let locations = host.references(&path, position);
    Value::Array(
        locations
            .iter()
            .map(|location| {
                let uri = Url::from_file_path(&location.file)
                    .map(|url| url.to_string())
                    .unwrap_or_default();
                json!({ "uri": uri, "range": range_json(location.range) })
            })
            .collect(),
    )
}

fn document_symbols(host: &mut dyn AnalysisHost, message: &Value) -> Value {
    let Some(uri) = message
        .get("params")
        .and_then(|params| params.get("textDocument"))
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
    else {
        return Value::Null;
    };
    let Ok(path) = uri_to_path(uri) else {
        return Value::Null;
    };
    let symbols = host.document_symbols(&path);
    Value::Array(symbols.iter().map(document_symbol_json).collect())
}

fn document_symbol_json(symbol: &javac_analysis_api::DocumentSymbol) -> Value {
    json!({
        "name": symbol.name,
        "kind": symbol_kind_number(symbol.kind),
        "range": range_json(symbol.range),
        "selectionRange": range_json(symbol.range),
        "children": symbol.children.iter().map(document_symbol_json).collect::<Vec<_>>(),
    })
}

fn hover(host: &mut dyn AnalysisHost, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    host.hover(&path, position)
        .map(|text| json!({ "contents": { "kind": "plaintext", "value": text } }))
        .unwrap_or(Value::Null)
}

fn request_position(message: &Value) -> Option<(PathBuf, Position)> {
    let params = message.get("params")?;
    let uri = params.get("textDocument")?.get("uri")?.as_str()?;
    let line = params.get("position")?.get("line")?.as_u64()? as u32;
    let character = params.get("position")?.get("character")?.as_u64()? as u32;
    let path = uri_to_path(uri).ok()?;
    Some((path, Position::new(line, character)))
}

fn symbol_kind_number(kind: SymbolKind) -> i32 {
    match kind {
        SymbolKind::Class => 7,
        SymbolKind::Interface => 11,
        SymbolKind::Enum => 13,
        SymbolKind::Record => 23,
        SymbolKind::Method => 2,
        SymbolKind::Field => 8,
        SymbolKind::LocalVariable => 13,
        SymbolKind::Parameter => 14,
        SymbolKind::Package => 4,
        SymbolKind::TypeParameter => 25,
    }
}

fn read_message(stdin: &mut impl BufRead) -> Option<Value> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().ok();
        }
    }
    let length = content_length?;
    let mut body = vec![0u8; length];
    stdin.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn respond(stdout: &mut impl Write, id: Option<Value>, result: Value) {
    send(
        stdout,
        json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result }),
    );
}

fn respond_error(stdout: &mut impl Write, id: Value, code: i32, message: &str) {
    send(
        stdout,
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    );
}

fn notify(stdout: &mut impl Write, method: &str, params: Value) {
    send(
        stdout,
        json!({ "jsonrpc": "2.0", "method": method, "params": params }),
    );
}

fn send(stdout: &mut impl Write, message: Value) {
    let body = serde_json::to_string(&message).unwrap_or_else(|_| "{}".to_string());
    let _ = write!(stdout, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = stdout.flush();
}
