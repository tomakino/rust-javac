# Java IDE Engine (javac-ide-core + ravac-lsp)

## Overview

Two crates:

- crates/javac-ide-core: a fork of the javac port restricted to analysis (parser, tree, code, comp, util, resources, file, class reader). Code generation is removed; the analysis session keeps compiler services alive across requests.
- crates/ravac-lsp: a stdio LSP server. It depends only on javac-analysis-api and lsp-types; the backend is injected as a dyn AnalysisHost.
- crates/javac-analysis-api: compiler-agnostic traits and plain data (Position, Range, Diagnostic, Symbol, CompletionItem, Location, DocumentSymbol, AnalysisHost).

The original compiler crate (crates/javac-core) is untouched.

## Architecture

    editor (VS Code / any LSP client)
        |  stdio JSON-RPC (Content-Length framing)
    ravac-lsp (src/main.rs: assembly; src/lib.rs: protocol)
        |  dyn AnalysisHost
    javac-analysis-api (trait + data)
        |
    javac-ide-core::analysis::AnalysisSession
        |  parse -> enter -> attribute (no generate)
    javac-ide-core compiler services (reused across requests)

## Capabilities

| LSP method | Status | Notes |
|---|---|---|
| initialize | yes | capabilities for completion/definition/hover/references/documentSymbol |
| initialized | yes | no-op |
| shutdown / exit | yes | |
| textDocument/didOpen | yes | analyzes the buffer, publishes diagnostics |
| textDocument/didChange | yes | full sync; incremental analysis (<1 ms on Gson) |
| textDocument/didSave | yes | includeText; re-analyzes |
| textDocument/didClose | yes | clears diagnostics |
| textDocument/publishDiagnostics | yes | structured JCDiagnostic: line, UTF-16 column, message, severity |
| textDocument/completion | yes | prefix filter, ranking (class > interface > method > field), project index |
| textDocument/definition | yes | local declaration, project index, JDK source (JAVA_HOME/lib/src.zip) |
| textDocument/hover | yes | class name / field / method signature (return and parameter types) |
| textDocument/references | yes | word-boundary search in current file and indexed project sources |
| textDocument/documentSymbol | yes | class -> members hierarchy with ranges |

## Running

Build:

    cargo build -p ravac-lsp

Start the server (any LSP client can spawn it):

    target/debug/ravac-lsp.exe --class-path <classes> [--source 17]

- --class-path: OS path separator separated list of class directories / jars.
- JDK source lookup uses JAVA_HOME/lib/src.zip.

## Tests

End-to-end smoke test (starts the server, exercises every method):

    py scripts/lsp/run_lsp_tests.py [class-path]

Expected: 11/11 checks passed.

Regression:

    cargo test -p javac-core --lib        # 750 tests
    cargo test -p javac-ide-core --lib    # 746 tests
    py scripts/check_gson.py              # 208/208 byte-for-byte

## Known limitations

- completion returns only symbols of the current file and source-backed project classes; inherited members (Object.clone etc.) still leak into empty-prefix results.
- references are textual word-boundary matches, not attributed symbol references.
- definition of class-path classes without sources (jars) returns null unless the JDK source archive has the file.
- hover for fields falls back to the qualified name when the field type is unresolved.
- diagnostics carry a single position (line + UTF-16 column), no length range.
- the analysis session is single-threaded; the LSP loop is synchronous.

## Layout

    crates/javac-ide-core/src/analysis.rs        session, cache, analyze loop
    crates/javac-ide-core/src/analysis/symbols.rs symbols + positional lookup
    crates/javac-ide-core/src/analysis/index.rs   project symbol index
    crates/javac-ide-core/src/analysis/host.rs    AnalysisHost implementation
    crates/javac-ide-core/src/analysis/positions.rs UTF-16 position helpers
    crates/javac-ide-core/src/analysis/diagnostics.rs JCDiagnostic conversion
    crates/javac-ide-core/src/analysis/jdk_sources.rs JDK src.zip lookup
    crates/javac-ide-core/src/analysis/type_names.rs  type rendering
    crates/ravac-lsp/src/lib.rs                   stdio JSON-RPC + method dispatch
