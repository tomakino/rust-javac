
//! Minimal LSP server for the analysis API.
//!
//! Depends only on javac-analysis-api and lsp-types; the backend is injected
//! as a `Box<dyn WorkspaceService>`. Protocol plumbing is a small stdio
//! JSON-RPC loop.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use javac_analysis_api::{
    Diagnostic, DocumentHighlightKind, FoldingRangeKind, Position, Range, SelectionRange, Severity,
    SymbolKind, WorkspaceService,
};
use lsp_types::Url;
use serde_json::{json, Value};

pub mod artifacts;

/// Build-system registry used by the server: concrete build systems first,
/// the core directory-scan fallback last.
pub fn project_registry() -> Vec<Box<dyn javac_ide_core::project::BuildSystemProvider>> {
    vec![
        Box::new(javac_build_maven::MavenProvider::new()),
        Box::new(javac_ide_core::project::DirScanProvider),
    ]
}

pub fn run(
    host: &mut Box<dyn WorkspaceService>,
    mut stdin: impl BufRead,
    mut stdout: impl Write,
) {
    let mut docs: HashMap<PathBuf, String> = HashMap::new();
    let mut artifacts: Option<artifacts::ArtifactBuilder> = None;
    loop {
        let Some(message) = read_message(&mut stdin) else { break };
        // Apply a finished artifact build exactly once, before serving the
        // next request: the analysis switches from source fallback to classes.
        if let Some(builder) = &artifacts
            && let Some(class_path) = builder.take_completion()
        {
            host.update_analysis_config(class_path, Vec::new());
            log_line("artifacts ready: analysis switched to the artifact class path");
        }
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let id = message.get("id").cloned();
        match method {
            "initialize" => {
                // Resolve the build system through the project layer and
                // rebuild the workspace. Initialization has no state to keep.
                if let Some(root) = initialize_root(&message) {
                    let output_dir = host.configuration().output_dir;
                    let model = javac_ide_core::project::load_with(
                        &project_registry(),
                        &root,
                        &javac_ide_core::project::LoadContext::default(),
                    );
                    *host = Box::new(javac_ide_core::workspace::Workspace::new_from_project(
                        &model,
                        output_dir,
                        None,
                    ));
                    let source_level = host.configuration().source_level;
                    let (sources, class_path, has_output, level) =
                        artifact_inputs(&model, &source_level);
                    let builder = artifacts::ArtifactBuilder::new(artifacts::ArtifactInput {
                        root: root.clone(),
                        sources: sources.clone(),
                        class_path,
                        source_level: level,
                        has_project_output: has_output,
                    });
                    if !has_output {
                        builder.start_full_build(sources);
                    }
                    artifacts = Some(builder);
                }
                respond(&mut stdout, id, json!({ "capabilities": capabilities() }));
            }
            "initialized" => {}
            "shutdown" => respond(&mut stdout, id, Value::Null),
            "exit" => break,
            "textDocument/didOpen" => {
                if let Some((uri, text)) = document_text(&message, "text") {
                    if let Ok(path) = uri_to_path(&uri) {
                        log_line(&format!("didOpen {} len={} head={:?}", uri, text.len(), &text[..text.len().min(120)]));
                        docs.insert(path.clone(), text.clone());
                        publish_diagnostics(&mut stdout, host.as_mut(), &uri, &path, &text);
                    }
                }
            }
            "textDocument/didChange" => {
                if let Some((uri, text)) = change_text(&message, &docs) {
                    if let Ok(path) = uri_to_path(&uri) {
                        log_line(&format!("didChange {} len={} head={:?}", uri, text.len(), &text[..text.len().min(120)]));
                        docs.insert(path.clone(), text.clone());
                        publish_diagnostics(&mut stdout, host.as_mut(), &uri, &path, &text);
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = document_uri(&message) {
                    if let Ok(path) = uri_to_path(&uri) {
                        docs.remove(&path);
                        host.invalidate(&[path]);
                    }
                    publish_empty(&mut stdout, &uri);
                }
            }
            "workspace/didChangeWatchedFiles" => {
                let paths: Vec<PathBuf> = message
                    .get("params")
                    .and_then(|params| params.get("changes"))
                    .and_then(Value::as_array)
                    .map(|changes| {
                        changes
                            .iter()
                            .filter_map(|change| change.get("uri").and_then(Value::as_str))
                            .filter_map(|uri| uri_to_path(uri).ok())
                            .collect()
                    })
                    .unwrap_or_default();
                host.invalidate(&paths);
            }
            "textDocument/didSave" => {
                if let Some(uri) = document_uri(&message) {
                    if let Ok(path) = uri_to_path(&uri) {
                        // Recompile the saved file and its dependent closure so
                        // referrers see the new classes; didChange never builds.
                        if let Some(builder) = &artifacts {
                            let t0 = std::time::Instant::now();
                            let mut files = host.dependent_closure(&path);
                            log_line(&format!("didSave: closure {}ms", t0.elapsed().as_millis()));
                            if files.is_empty() {
                                files.push(path.clone());
                            }
                            let started = std::time::Instant::now();
                            let ok = builder.build_files(&files);
                            log_line(&format!("didSave: build {}ms", started.elapsed().as_millis()));
                            for file in &files {
                                host.invalidate_analysis(file);
                            }
                            log_line(&format!("didSave: invalidate {}ms", started.elapsed().as_millis()));
                            log_line(&format!(
                                "artifacts: rebuilt {} file(s) after save, ok={ok}, {}ms",
                                files.len(),
                                started.elapsed().as_millis()
                            ));
                        }
                        let text = message
                            .get("params")
                            .and_then(|params| params.get("text"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or_else(|| docs.get(&path).cloned());
                        if let Some(text) = text {
                            let t2 = std::time::Instant::now();
                            // didChange already published diagnostics for this
                            // text; re-diagnosing on save would recompute the
                            // same result (and re-run the index update).
                            let on_disk = std::fs::read_to_string(&path).ok();
                            log_line(&format!(
                                "didSave: text-check {}ms same={}",
                                t2.elapsed().as_millis(),
                                on_disk.as_deref() == Some(text.as_str())
                            ));
                            if on_disk.as_deref() != Some(text.as_str()) {
                                docs.insert(path.clone(), text.clone());
                                publish_diagnostics(&mut stdout, host.as_mut(), &uri, &path, &text);
                            }
                        }
                    }
                }
            }
            "$/cancelRequest" => {}
            "textDocument/completion" => {
                if let Some(params) = message.get("params") {
                    let position = params.get("position").cloned().unwrap_or(Value::Null);
                    let line_number = position.get("line").and_then(Value::as_u64);
                    let line_text = params
                        .get("textDocument")
                        .and_then(|doc| doc.get("uri"))
                        .and_then(Value::as_str)
                        .and_then(|uri| uri_to_path(uri).ok())
                        .and_then(|path| docs.get(&path).cloned())
                        .and_then(|text| {
                            line_number.and_then(|line| {
                                text.lines().nth(line as usize).map(str::to_string)
                            })
                        });
                    log_line(&format!("completion position={position} line={line_text:?}"));
                }
                respond(&mut stdout, id, completion(host.as_mut(), &message));
            }
            "textDocument/formatting" => {
                respond(&mut stdout, id, formatting(host.as_mut(), &message, &docs));
            }
            "textDocument/onTypeFormatting" => {
                respond(&mut stdout, id, on_type_formatting(host.as_mut(), &message));
            }
            "textDocument/documentLink" => {
                respond(&mut stdout, id, document_links(host.as_mut(), &message));
            }
            "textDocument/semanticTokens/full" => {
                respond(&mut stdout, id, semantic_tokens(host.as_mut(), &message));
            }
            "textDocument/codeLens" => {
                respond(&mut stdout, id, code_lens(host.as_mut(), &message));
            }
            "workspace/executeCommand" => {
                respond(&mut stdout, id, execute_command(host.as_mut(), &message));
            }
            "textDocument/definition" => {
                respond(&mut stdout, id, definition(host.as_mut(), &message));
            }
            "textDocument/typeDefinition" => {
                respond(&mut stdout, id, type_definition(host.as_mut(), &message));
            }
            "textDocument/implementation" => {
                respond(&mut stdout, id, implementations(host.as_mut(), &message));
            }
            "textDocument/signatureHelp" => {
                respond(&mut stdout, id, signature_help(host.as_mut(), &message));
            }
            "textDocument/inlayHint" => {
                respond(&mut stdout, id, inlay_hints(host.as_mut(), &message));
            }
            "ravac/superMethods" => {
                respond(&mut stdout, id, super_methods_request(host.as_mut(), &message));
            }
            "textDocument/documentHighlight" => {
                respond(&mut stdout, id, document_highlight(host.as_mut(), &message));
            }
            "textDocument/foldingRange" => {
                respond(&mut stdout, id, folding_ranges(host.as_mut(), &message));
            }
            "textDocument/selectionRange" => {
                respond(&mut stdout, id, selection_range(host.as_mut(), &message));
            }
            "textDocument/codeAction" => {
                respond(&mut stdout, id, code_action(host.as_mut(), &message));
            }
            "textDocument/prepareTypeHierarchy" => {
                respond(&mut stdout, id, prepare_type_hierarchy(host.as_mut(), &message));
            }
            "typeHierarchy/supertypes" => {
                respond(&mut stdout, id, type_hierarchy_supertypes(host.as_mut(), &message));
            }
            "typeHierarchy/subtypes" => {
                respond(&mut stdout, id, type_hierarchy_subtypes(host.as_mut(), &message));
            }
            "textDocument/prepareCallHierarchy" => {
                respond(&mut stdout, id, prepare_call_hierarchy(host.as_mut(), &message));
            }
            "callHierarchy/incomingCalls" => {
                respond(&mut stdout, id, incoming_calls(host.as_mut(), &message));
            }
            "callHierarchy/outgoingCalls" => {
                respond(&mut stdout, id, outgoing_calls(host.as_mut(), &message));
            }
            "textDocument/hover" => {
                respond(&mut stdout, id, hover(host.as_mut(), &message));
            }
            "textDocument/documentSymbol" => {
                respond(&mut stdout, id, document_symbols(host.as_mut(), &message));
            }
            "workspace/symbol" => {
                respond(&mut stdout, id, workspace_symbols(host.as_mut(), &message));
            }
            "ravac/indexStats" => {
                respond(
                    &mut stdout,
                    id,
                    json!({
                        "generation": host.index_generation(),
                        "building": host.is_index_building(),
                    }),
                );
            }
            "ravac/artifactStats" => {
                let value = match &artifacts {
                    Some(builder) => builder.stats().to_json(),
                    None => json!({
                        "status": "skipped",
                        "dir": "",
                        "files": 0,
                        "classes": 0,
                        "error": Value::Null,
                    }),
                };
                respond(&mut stdout, id, value);
            }
            "textDocument/references" => {
                respond(&mut stdout, id, references(host.as_mut(), &message));
            }
            "textDocument/rename" => {
                respond(&mut stdout, id, rename(host.as_mut(), &message));
            }
            "textDocument/prepareRename" => {
                respond(&mut stdout, id, prepare_rename(host.as_mut(), &message));
            }
            _ => {
                if let Some(id) = id {
                    respond_error(&mut stdout, id, -32601, "method not found");
                }
            }
        }
    }
}

/// Project root advertised by the client (rootUri or first workspace folder).
fn initialize_root(message: &Value) -> Option<PathBuf> {
    let params = message.get("params")?;
    let uri = params
        .get("rootUri")
        .and_then(Value::as_str)
        .or_else(|| {
            params
                .get("workspaceFolders")?
                .as_array()?
                .first()?
                .get("uri")?
                .as_str()
        })?;
    uri_to_path(uri).ok()
}

fn capabilities() -> Value {
    json!({
        "textDocumentSync": { "openClose": true, "change": 1, "save": { "includeText": true } },
        "completionProvider": { "triggerCharacters": [".", ":"] },
            "documentFormattingProvider": true,
        "documentOnTypeFormattingProvider": {
            "firstTriggerCharacter": "}",
            "moreTriggerCharacter": [";", "\n"]
        },
        "documentLinkProvider": { "resolveProvider": false },
        "semanticTokensProvider": {
            "legend": {
                "tokenTypes": javac_analysis_api::SEMANTIC_TOKEN_TYPES.to_vec(),
                "tokenModifiers": javac_analysis_api::SEMANTIC_TOKEN_MODIFIERS.to_vec()
            },
            "full": { "delta": false }
        },
        "codeLensProvider": { "resolveProvider": false },
        "executeCommandProvider": {
            "commands": ["ravac.organizeImports", "ravac.showReferences"]
        },
        "definitionProvider": true,
        "typeDefinitionProvider": true,
        "implementationProvider": true,
        "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
        "inlayHintProvider": { "resolveProvider": false },
        "codeActionProvider": true,
        "typeHierarchyProvider": true,
        "callHierarchyProvider": true,
        "hoverProvider": true,
        "documentHighlightProvider": true,
        "foldingRangeProvider": true,
        "selectionRangeProvider": true,
        "documentSymbolProvider": true,
        "workspaceSymbolProvider": true,
        "referencesProvider": true,
        "renameProvider": { "prepareProvider": true }
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

/// Debug log for server-side request tracing.
fn log_line(message: &str) {
    use std::io::Write as _;
    let path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("lsp.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("lsp.log"));
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{message}");
    }
}

fn change_text(
    message: &Value,
    docs: &std::collections::HashMap<PathBuf, String>,
) -> Option<(String, String)> {
    let params = message.get("params")?;
    let uri = params.get("textDocument")?.get("uri")?.as_str()?.to_string();
    let path = uri_to_path(&uri).ok()?;
    let changes = params.get("contentChanges")?.as_array()?;
    let mut text = docs.get(&path)?.clone();
    for change in changes {
        let new = change.get("text")?.as_str()?;
        match change.get("range") {
            Some(range) => {
                let start = offset_in(&text, range.get("start")?)?;
                let end = offset_in(&text, range.get("end")?)?;
                if start <= end && end <= text.len() {
                    text.replace_range(start..end, new);
                }
            }
            None => text = new.to_string(),
        }
    }
    Some((uri, text))
}

/// Byte offset of an LSP position (0-based line, UTF-16 column).
fn offset_in(text: &str, position: &Value) -> Option<usize> {
    let line = position.get("line")?.as_u64()? as usize;
    let character = position.get("character")?.as_u64()? as usize;
    let mut offset = 0;
    for (index, current) in text.split_inclusive(char::from(10)).enumerate() {
        if index == line {
            let mut column = 0;
            for (byte, ch) in current.char_indices() {
                if column >= character {
                    return Some(offset + byte);
                }
                column += ch.len_utf16();
            }
            return Some(offset + current.len());
        }
        offset += current.len();
    }
    Some(text.len())
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
    host: &mut dyn WorkspaceService,
    uri: &str,
    path: &PathBuf,
    text: &str,
) {
    let diagnostics = host.diagnostics(path, Some(text));
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
        "message": diagnostic.message,
        "code": diagnostic.code,
        "source": "ravac-lsp",
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

fn inlay_hints(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(params) = message.get("params") else {
        return Value::Null;
    };
    let Some(uri) = params
        .get("textDocument")
        .and_then(|document| document.get("uri"))
        .and_then(Value::as_str)
    else {
        return Value::Null;
    };
    let Ok(path) = uri_to_path(uri) else {
        return Value::Null;
    };
    let start = params
        .get("range")
        .and_then(|range| range.get("start"))
        .and_then(|position| {
            Some((
                position.get("line")?.as_u64()? as u32,
                position.get("character")?.as_u64()? as u32,
            ))
        });
    let end = params
        .get("range")
        .and_then(|range| range.get("end"))
        .and_then(|position| {
            Some((
                position.get("line")?.as_u64()? as u32,
                position.get("character")?.as_u64()? as u32,
            ))
        });
    let hints: Vec<Value> = host
        .inlay_hints(&path)
        .into_iter()
        .filter(|hint| {
            let key = (hint.position.line, hint.position.column);
            start.map_or(true, |start| key >= start) && end.map_or(true, |end| key <= end)
        })
        .map(|hint| {
            let kind = match hint.kind {
                javac_analysis_api::InlayHintKind::Type => 1,
                javac_analysis_api::InlayHintKind::Parameter => 2,
            };
            json!({
                "position": {
                    "line": hint.position.line,
                    "character": hint.position.column,
                },
                "label": hint.label,
                "kind": kind,
                "paddingLeft": hint.padding_left,
                "paddingRight": hint.padding_right,
            })
        })
        .collect();
    Value::Array(hints)
}

fn completion(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    let items: Vec<Value> = host
        .completion(&path, position)
        .into_iter()
        .map(|item| {
            let mut value = json!({
                "label": item.label,
                "kind": symbol_kind_number(item.kind),
                "detail": item.detail,
            });
            if let Some(filter_text) = item.filter_text {
                value["filterText"] = json!(filter_text);
            }
            if let Some(label_detail) = item.label_detail {
                value["labelDetails"] = json!({ "detail": label_detail });
            }
            if let Some(sort_text) = item.sort_text {
                value["sortText"] = json!(sort_text);
            }
            if let Some(insert_text) = item.insert_text {
                let insert_format = match item.insert_text_format {
                    javac_analysis_api::InsertTextFormat::PlainText => 1,
                    javac_analysis_api::InsertTextFormat::Snippet => 2,
                };
                if let Some(prefix_len) = item.replace_prefix_len {
                    let start = position.column.saturating_sub(prefix_len);
                    value["textEdit"] = json!({
                        "range": {
                            "start": { "line": position.line, "character": start },
                            "end": { "line": position.line, "character": position.column }
                        },
                        "newText": insert_text
                    });
                } else {
                    value["insertText"] = json!(insert_text);
                }
                value["insertTextFormat"] = json!(insert_format);
            }
            if let Some(documentation) = item.documentation {
                value["documentation"] = json!({ "kind": "markdown", "value": documentation });
            }
            if !item.additional_text_edits.is_empty() {
                value["additionalTextEdits"] = json!(item
                    .additional_text_edits
                    .iter()
                    .map(|edit| json!({
                        "range": range_json(edit.range),
                        "newText": edit.new_text,
                    }))
                    .collect::<Vec<_>>());
            }
            value
        })
        .collect();
    Value::Array(items)
}

fn call_item_json(item: &javac_analysis_api::CallHierarchyItem) -> Value {
    let uri = Url::from_file_path(&item.file)
        .map(|url| url.to_string())
        .unwrap_or_default();
    json!({
        "name": item.name,
        "kind": 6,
        "uri": uri,
        "range": range_json(item.range),
        "selectionRange": range_json(item.range),
        "detail": "",
    })
}

fn call_item_from_params(message: &Value) -> Option<javac_analysis_api::CallHierarchyItem> {
    let item = message.get("params")?.get("item")?;
    let name = item.get("name")?.as_str()?.to_string();
    let uri = item.get("uri")?.as_str()?;
    let file = uri_to_path(uri).ok()?;
    let start = item.get("range")?.get("start")?;
    let line = start.get("line")?.as_u64()? as u32;
    let character = start.get("character")?.as_u64()? as u32;
    Some(javac_analysis_api::CallHierarchyItem {
        name,
        file,
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, character)),
    })
}

fn prepare_call_hierarchy(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    match host.prepare_call_hierarchy(&path, position) {
        Some(item) => Value::Array(vec![call_item_json(&item)]),
        None => Value::Null,
    }
}

fn incoming_calls(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(item) = call_item_from_params(message) else {
        return Value::Null;
    };
    Value::Array(
        host.incoming_calls(&item)
            .iter()
            .map(|call| {
                json!({
                    "from": call_item_json(&call.item),
                    "fromRanges": call.from_ranges.iter().map(|range| range_json(*range)).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

fn outgoing_calls(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(item) = call_item_from_params(message) else {
        return Value::Null;
    };
    Value::Array(
        host.outgoing_calls(&item)
            .iter()
            .map(|call| {
                json!({
                    "to": call_item_json(&call.item),
                    "fromRanges": call.from_ranges.iter().map(|range| range_json(*range)).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

fn type_hierarchy_json(item: &javac_analysis_api::TypeHierarchyItem) -> Value {
    let uri = Url::from_file_path(&item.file)
        .map(|url| url.to_string())
        .unwrap_or_default();
    json!({
        "name": item.name,
        "kind": symbol_kind_number(item.kind),
        "uri": uri,
        "range": range_json(item.range),
        "selectionRange": range_json(item.range),
    })
}

fn prepare_type_hierarchy(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    match host.prepare_type_hierarchy(&path, position) {
        Some(item) => Value::Array(vec![type_hierarchy_json(&item)]),
        None => Value::Null,
    }
}

fn item_from_params(host: &mut dyn WorkspaceService, message: &Value) -> Option<javac_analysis_api::TypeHierarchyItem> {
    let item = message.get("params")?.get("item")?;
    let name = item.get("name")?.as_str()?.to_string();
    let uri = item.get("uri")?.as_str()?;
    let path = uri_to_path(uri).ok()?;
    let range = item.get("range")?;
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? as u32;
    let character = start.get("character")?.as_u64()? as u32;
    let item = javac_analysis_api::TypeHierarchyItem {
        name,
        kind: javac_analysis_api::SymbolKind::Class,
        file: path,
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, character)),
    };
    let _ = host;
    Some(item)
}

fn type_hierarchy_supertypes(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(item) = item_from_params(host, message) else {
        return Value::Null;
    };
    Value::Array(host.supertypes(&item).iter().map(type_hierarchy_json).collect())
}

fn type_hierarchy_subtypes(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(item) = item_from_params(host, message) else {
        return Value::Null;
    };
    Value::Array(host.subtypes(&item).iter().map(type_hierarchy_json).collect())
}

fn code_action(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(path) = message
        .get("params")
        .and_then(|params| params.get("textDocument"))
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
        .and_then(|uri| uri_to_path(uri).ok())
    else {
        return Value::Null;
    };
    let Some(start) = message
        .get("params")
        .and_then(|params| params.get("range"))
        .and_then(|range| range.get("start"))
    else {
        return Value::Null;
    };
    let line = start.get("line").and_then(Value::as_u64).unwrap_or(0) as u32;
    let character = start.get("character").and_then(Value::as_u64).unwrap_or(0) as u32;
    let position = javac_analysis_api::Position::new(line, character);
    let mut actions = host.code_actions(&path, position);
    if let Some(edit) = host.organize_imports(&path).first() {
        actions.push(javac_analysis_api::CodeAction {
            title: "Organize imports".to_string(),
            edits: vec![edit.clone()],
            diagnostics: Vec::new(),
        });
    }
    let uri = Url::from_file_path(&path)
        .map(|url| url.to_string())
        .unwrap_or_default();
    Value::Array(
        actions
            .iter()
            .map(|action| {
                let edits: Vec<Value> = action
                    .edits
                    .iter()
                    .map(|edit| {
                        json!({
                            "range": range_json(edit.range),
                            "newText": edit.new_text,
                        })
                    })
                    .collect();
                json!({
                    "title": action.title,
                    "kind": if action.title == "Organize imports" {
                        "source.organizeImports"
                    } else if action.title.starts_with("Generate") {
                        "refactor"
                    } else {
                        "quickfix"
                    },
                    "diagnostics": action
                        .diagnostics
                        .iter()
                        .map(|diagnostic| {
                            json!({
                                "range": range_json(diagnostic.range),
                                "severity": severity_number(diagnostic.severity),
                                "message": diagnostic.message,
                                "code": diagnostic.code,
                            })
                        })
                        .collect::<Vec<Value>>(),
                    "edit": { "changes": { uri.clone(): edits } },
                })
            })
            .collect(),
    )
}

fn semantic_tokens(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(path) = message
        .get("params")
        .and_then(|params| params.get("textDocument"))
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
        .and_then(|uri| uri_to_path(uri).ok())
    else {
        return Value::Null;
    };
    json!({ "data": host.semantic_tokens(&path) })
}

fn code_lens(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(path) = message
        .get("params")
        .and_then(|params| params.get("textDocument"))
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
        .and_then(|uri| uri_to_path(uri).ok())
    else {
        return Value::Null;
    };
    let lenses = host.code_lenses(&path);
    Value::Array(
        lenses
            .iter()
            .map(|lens| {
                json!({
                    "range": range_json(lens.range),
                    "command": {
                        "title": lens.title,
                        "command": lens.command,
                        "arguments": [lens.uri, { "line": lens.line, "character": lens.character }],
                    }
                })
            })
            .collect(),
    )
}

fn execute_command(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(params) = message.get("params") else {
        return Value::Null;
    };
    let Some(command) = params.get("command").and_then(Value::as_str) else {
        return Value::Null;
    };
    let Some(uri) = params
        .get("arguments")
        .and_then(Value::as_array)
        .and_then(|arguments| arguments.first())
        .and_then(Value::as_str)
    else {
        return Value::Null;
    };
    let Ok(path) = uri_to_path(uri) else {
        return Value::Null;
    };
    match command {
        "ravac.organizeImports" => {
            let edits = host.organize_imports(&path);
            Value::Array(
                edits
                    .iter()
                    .map(|edit| json!({ "range": range_json(edit.range), "newText": edit.new_text }))
                    .collect(),
            )
        }
        _ => Value::Null,
    }
}

fn on_type_formatting(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(path) = message
        .get("params")
        .and_then(|params| params.get("textDocument"))
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
        .and_then(|uri| uri_to_path(uri).ok())
    else {
        return Value::Null;
    };
    let Some(position) = message.get("params").and_then(|params| params.get("position")) else {
        return Value::Null;
    };
    let line = position.get("line").and_then(Value::as_u64).unwrap_or(0) as u32;
    let column = position.get("character").and_then(Value::as_u64).unwrap_or(0) as u32;
    let Some(ch) = message
        .get("params")
        .and_then(|params| params.get("ch"))
        .and_then(Value::as_str)
        .and_then(|ch| ch.chars().next())
    else {
        return Value::Null;
    };
    let edits = host.on_type_formatting(
        &path,
        javac_analysis_api::Position::new(line, column),
        ch,
    );
    Value::Array(
        edits
            .iter()
            .map(|edit| json!({ "range": range_json(edit.range), "newText": edit.new_text }))
            .collect(),
    )
}

fn document_links(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(path) = message
        .get("params")
        .and_then(|params| params.get("textDocument"))
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
        .and_then(|uri| uri_to_path(uri).ok())
    else {
        return Value::Null;
    };
    let links = host.document_links(&path);
    Value::Array(
        links
            .iter()
            .map(|link| json!({ "range": range_json(link.range), "target": link.target }))
            .collect(),
    )
}

fn formatting(
    host: &mut dyn WorkspaceService,
    message: &Value,
    docs: &std::collections::HashMap<PathBuf, String>,
) -> Value {
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
    let Some(text) = docs.get(&path) else {
        return Value::Null;
    };
    let Some(formatted) = host.format(&path, text) else {
        return Value::Null;
    };
    let last_line = text.lines().count().saturating_sub(1) as u32;
    let last_column = text
        .lines()
        .last()
        .map(|line| line.chars().map(|c| c.len_utf16() as u32).sum())
        .unwrap_or(0);
    json!([{
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": last_line, "character": last_column},
        },
        "newText": formatted,
    }])
}

fn signature_help(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    match host.signature_help(&path, position) {
        Some(help) => {
            let signatures: Vec<Value> = help
                .signatures
                .iter()
                .map(|signature| {
                    json!({
                        "label": signature.label,
                        "parameters": signature.parameters.iter().map(|parameter| json!({ "label": parameter })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            json!({
                "signatures": signatures,
                "activeSignature": help.active_signature,
                "activeParameter": help.active_parameter,
            })
        }
        None => Value::Null,
    }
}

fn implementations(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    let locations = host.implementation(&path, position);
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

fn type_definition(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    match host.type_definition(&path, position) {
        Some(location) => {
            let uri = Url::from_file_path(&location.file)
                .map(|url| url.to_string())
                .unwrap_or_default();
            json!({ "uri": uri, "range": range_json(location.range) })
        }
        None => Value::Null,
    }
}

fn definition(host: &mut dyn WorkspaceService, message: &Value) -> Value {
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

fn rename(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    let Some(new_name) = message
        .get("params")
        .and_then(|params| params.get("newName"))
        .and_then(Value::as_str)
    else {
        return Value::Null;
    };
    let edits = host.rename_workspace(&path, position, new_name);
    let mut changes = serde_json::Map::new();
    for edit in edits {
        let uri = Url::from_file_path(&edit.file)
            .map(|url| url.to_string())
            .unwrap_or_default();
        let entry = changes
            .entry(uri)
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(edits) = entry {
            edits.push(json!({
                "range": range_json(edit.edit.range),
                "newText": edit.edit.new_text,
            }));
        }
    }
    json!({ "changes": changes })
}

fn prepare_rename(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    match host.prepare_rename(&path, position) {
        Some(range) => json!({ "range": range_json(range) }),
        None => Value::Null,
    }
}

fn document_highlight(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    let highlights = host.document_highlight(&path, position);
    Value::Array(
        highlights
            .iter()
            .map(|highlight| {
                let mut value = json!({ "range": range_json(highlight.range) });
                if let Some(kind) = highlight.kind {
                    value["kind"] = json!(match kind {
                        DocumentHighlightKind::Text => 1,
                        DocumentHighlightKind::Read => 2,
                        DocumentHighlightKind::Write => 3,
                    });
                }
                value
            })
            .collect(),
    )
}

fn folding_ranges(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(params) = message.get("params") else {
        return Value::Array(Vec::new());
    };
    let Some(uri) = params
        .get("textDocument")
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
    else {
        return Value::Array(Vec::new());
    };
    let Ok(path) = uri_to_path(uri) else {
        return Value::Array(Vec::new());
    };
    let ranges = host.folding_ranges(&path);
    Value::Array(
        ranges
            .iter()
            .map(|range| {
                let mut value = json!({
                    "startLine": range.start_line,
                    "endLine": range.end_line,
                });
                if let Some(kind) = range.kind {
                    value["kind"] = json!(match kind {
                        FoldingRangeKind::Comment => "comment",
                        FoldingRangeKind::Imports => "imports",
                        FoldingRangeKind::Region => "region",
                    });
                }
                value
            })
            .collect(),
    )
}

fn super_methods_request(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    let locations = host.super_methods(&path, position);
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

fn selection_range(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some(params) = message.get("params") else {
        return Value::Array(Vec::new());
    };
    let Some(uri) = params
        .get("textDocument")
        .and_then(|doc| doc.get("uri"))
        .and_then(Value::as_str)
    else {
        return Value::Array(Vec::new());
    };
    let Ok(path) = uri_to_path(uri) else {
        return Value::Array(Vec::new());
    };
    let positions: Vec<Position> = params
        .get("positions")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| {
                    Some(Position::new(
                        value.get("line")?.as_u64()? as u32,
                        value.get("character")?.as_u64()? as u32,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    if positions.is_empty() {
        return Value::Array(Vec::new());
    }
    let ranges = host.selection_ranges(&path, &positions);
    Value::Array(
        ranges
            .iter()
            .map(selection_range_json)
            .collect::<Vec<_>>(),
    )
}

fn selection_range_json(range: &SelectionRange) -> Value {
    let parent = range
        .parent
        .as_ref()
        .map(|parent| selection_range_json(parent))
        .unwrap_or(Value::Null);
    json!({
        "range": range_json(range.range),
        "parent": parent,
    })
}

fn references(host: &mut dyn WorkspaceService, message: &Value) -> Value {
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


fn workspace_symbols(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let query = message
        .get("params")
        .and_then(|params| params.get("query"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let symbols = host.workspace_symbols(query);
    Value::Array(
        symbols
            .iter()
            .map(|symbol| {
                let uri = Url::from_file_path(&symbol.location.file)
                    .map(|url| url.to_string())
                    .unwrap_or_default();
                json!({
                    "name": symbol.name,
                    "kind": symbol_kind_number(symbol.kind),
                    "location": { "uri": uri, "range": range_json(symbol.location.range) },
                    "containerName": symbol.container_name,
                })
            })
            .collect(),
    )
}

fn document_symbols(host: &mut dyn WorkspaceService, message: &Value) -> Value {
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

fn hover(host: &mut dyn WorkspaceService, message: &Value) -> Value {
    let Some((path, position)) = request_position(message) else {
        return Value::Null;
    };
    host.hover(&path, position)
        .map(|text| json!({ "contents": { "kind": "markdown", "value": text } }))
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
        SymbolKind::Keyword => 14,
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

/// Recursively collect `.java` files under `roots`.
fn collect_java_sources(roots: &[PathBuf], out: &mut Vec<PathBuf>) {
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_java_sources(std::slice::from_ref(&path), out);
            } else if path.extension().is_some_and(|extension| extension == "java") {
                out.push(path);
            }
        }
    }
}

/// Files and class path for artifact maintenance plus the project-output flag.
fn artifact_inputs(
    model: &javac_ide_core::project::model::ProjectModel,
    fallback_source_level: &str,
) -> (Vec<PathBuf>, Vec<PathBuf>, bool, String) {
    let mut sources = Vec::new();
    let mut class_path = Vec::new();
    let mut has_output = false;
    for module in &model.modules {
        for set in &module.source_sets {
            collect_java_sources(&set.roots, &mut sources);
            for entry in &set.classpath {
                if !class_path.contains(entry) {
                    class_path.push(entry.clone());
                }
            }
            if let Some(output) = &set.output
                && output.is_dir()
                && contains_class_file(output)
            {
                has_output = true;
                if !class_path.contains(output) {
                    class_path.push(output.clone());
                }
            }
        }
    }
    sources.sort();
    sources.dedup();
    class_path.sort();
    class_path.dedup();
    let level = model
        .modules
        .iter()
        .find_map(|module| module.source_level.clone())
        .unwrap_or_else(|| fallback_source_level.to_string());
    (sources, class_path, has_output, level)
}

/// Whether `dir` contains any compiled class (recursively).
fn contains_class_file(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if contains_class_file(&path) {
                return true;
            }
        } else if path.extension().is_some_and(|extension| extension == "class") {
            return true;
        }
    }
    false
}