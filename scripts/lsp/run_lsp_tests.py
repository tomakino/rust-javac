
"""End-to-end LSP smoke test for ravac-lsp.

Usage:
    py scripts/lsp/run_lsp_tests.py [class-path]

Starts target/debug/ravac-lsp.exe, exercises every implemented method and
prints PASS/FAIL for each scenario.
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
DEFAULT_CLASS_PATH = ROOT / "target" / "tmp" / "perf" / "javac"

results = []


def check(name, ok, detail=""):
    results.append((name, ok, detail))
    print(("PASS  " if ok else "FAIL  ") + name + (("  (" + detail + ")") if detail else ""))


def main():
    class_path = sys.argv[1] if len(sys.argv) > 1 else str(DEFAULT_CLASS_PATH)
    (ROOT / "target" / "ravac-lsp").mkdir(parents=True, exist_ok=True)
    proc = subprocess.Popen(
        [str(EXE), "--class-path", class_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    def send(message):
        body = json.dumps(message).encode()
        proc.stdin.write(b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)
        proc.stdin.flush()

    def read():
        headers = b""
        while b"\r\n\r\n" not in headers:
            chunk = proc.stdout.read(1)
            if not chunk:
                return None
            headers += chunk
        length = int(headers.split(b"Content-Length: ")[1].split(b"\r\n")[0])
        return json.loads(proc.stdout.read(length))

    send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
    init = read()
    capabilities = init["result"]["capabilities"]
    check(
        "initialize capabilities",
        all(
            capabilities.get(key)
            for key in (
                "completionProvider",
                "definitionProvider",
                "hoverProvider",
                "documentSymbolProvider",
                "referencesProvider",
            )
        ),
    )

    uri = (ROOT / "target" / "tmp" / "LspSmoke.java").as_uri()
    text = "class LspSmoke { int bar; void baz() { bar = 1; } }\n"
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "java",
                    "version": 1,
                    "text": text,
                }
            },
        }
    )
    diagnostics = read()["params"]["diagnostics"]
    check("didOpen clean diagnostics", len(diagnostics) == 0, str(len(diagnostics)))

    send(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/completion",
            "params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 32}},
        }
    )
    labels = [item["label"] for item in (read().get("result") or [])]
    check("completion prefix filter", "baz" in labels and "bar" in labels, str(labels))

    send(
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "textDocument/hover",
            "params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 22}},
        }
    )
    hover = read()
    value = ((hover.get("result") or {}).get("contents") or {}).get("value", "")
    check("hover", "bar" in value, value)

    send(
        {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "textDocument/definition",
            "params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 22}},
        }
    )
    definition = read().get("result")
    check(
        "definition",
        definition is not None and definition.get("range", {}).get("start", {}).get("line") == 0,
        json.dumps(definition)[:80] if definition else "null",
    )

    send(
        {
            "jsonrpc": "2.0",
            "id": 5,
            "method": "textDocument/references",
            "params": {
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": 22},
                "context": {"includeDeclaration": True},
            },
        }
    )
    references = read().get("result") or []
    check("references", len(references) >= 2, str(len(references)))

    send(
        {
            "jsonrpc": "2.0",
            "id": 6,
            "method": "textDocument/documentSymbol",
            "params": {"textDocument": {"uri": uri}},
        }
    )
    symbols = read().get("result") or []
    children = symbols[0].get("children", []) if symbols else []
    check(
        "documentSymbol",
        bool(symbols) and any(child.get("name") == "bar" for child in children),
        str([child.get("name") for child in children]),
    )

    broken = "class LspSmoke { void f() { int x = ; } }\n"
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {"uri": uri, "version": 2},
                "contentChanges": [{"text": broken}],
            },
        }
    )
    check("didChange diagnostics", len(read()["params"]["diagnostics"]) == 1)

    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": {"textDocument": {"uri": uri}, "text": broken},
        }
    )
    check("didSave diagnostics", len(read()["params"]["diagnostics"]) == 1)

    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {"textDocument": {"uri": uri}},
        }
    )
    check("didClose clears diagnostics", len(read()["params"]["diagnostics"]) == 0)

    # Auto-import: completing an unimported JDK type carries additionalTextEdits.
    import_uri = (ROOT / "target" / "tmp" / "AutoImportSmoke.java").as_uri()
    import_text = "class AutoImportSmoke { void f() { new ArrayLis } }\n"
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": import_uri,
                    "languageId": "java",
                    "version": 1,
                    "text": import_text,
                }
            },
        }
    )
    read()
    import_pos = import_text.index("ArrayLis") + len("ArrayLis")
    send(
        {
            "jsonrpc": "2.0",
            "id": 8,
            "method": "textDocument/completion",
            "params": {
                "textDocument": {"uri": import_uri},
                "position": {"line": 0, "character": import_pos},
            },
        }
    )
    auto_items = read().get("result") or []
    auto_item = next(
        (item for item in auto_items if item.get("label") == "ArrayList"),
        None,
    )
    auto_edits = (auto_item or {}).get("additionalTextEdits") or []
    check(
        "completion auto-import",
        any(edit.get("newText") == "import java.util.ArrayList;\n" for edit in auto_edits),
        json.dumps(auto_edits)[:120],
    )
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {"textDocument": {"uri": import_uri}},
        }
    )
    read()

    # Inlay hints: inferred type of a var declaration.
    inlay_uri = (ROOT / "target" / "tmp" / "InlaySmoke.java").as_uri()
    inlay_text = 'class InlaySmoke { void f() { var value = "x"; new StringBuffer(16); } }\n'
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": inlay_uri,
                    "languageId": "java",
                    "version": 1,
                    "text": inlay_text,
                }
            },
        }
    )
    read()
    send(
        {
            "jsonrpc": "2.0",
            "id": 9,
            "method": "textDocument/inlayHint",
            "params": {
                "textDocument": {"uri": inlay_uri},
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 1, "character": 0},
                },
            },
        }
    )
    inlay_hints = read().get("result") or []
    check(
        "inlay hint var type",
        any(hint.get("label") == ": String" for hint in inlay_hints),
        json.dumps(inlay_hints)[:160],
    )
    check(
        "inlay hint parameter name",
        any(hint.get("label") == "capacity:" for hint in inlay_hints),
        json.dumps(inlay_hints)[:160],
    )
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {"textDocument": {"uri": inlay_uri}},
        }
    )
    read()

    send({"jsonrpc": "2.0", "id": 7, "method": "shutdown", "params": {}})
    read()
    send({"jsonrpc": "2.0", "method": "exit", "params": {}})
    proc.wait(timeout=10)
    check("exit code", proc.returncode == 0, str(proc.returncode))

    passed = sum(1 for _, ok, _ in results if ok)
    print("")
    print(str(passed) + "/" + str(len(results)) + " checks passed")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
