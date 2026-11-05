
"""Minimal LSP client for the editor simulator."""

import json
import subprocess
import time
from pathlib import Path


class LspClient:
    def __init__(self, exe, class_path):
        self.proc = subprocess.Popen(
            [str(exe), "--class-path", str(class_path)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.next_id = 1
        self.diagnostics = {}
        self.pending = []

    def _send(self, message):
        body = json.dumps(message).encode()
        self.proc.stdin.write(
            b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body
        )
        self.proc.stdin.flush()

    def _read_message(self):
        headers = b""
        while b"\r\n\r\n" not in headers:
            chunk = self.proc.stdout.read(1)
            if not chunk:
                return None
            headers += chunk
        length = int(headers.split(b"Content-Length: ")[1].split(b"\r\n")[0])
        body = self.proc.stdout.read(length)
        return json.loads(body)

    def _pump(self):
        message = self._read_message()
        if message is None:
            raise RuntimeError("server closed the connection")
        if message.get("method") == "textDocument/publishDiagnostics":
            params = message["params"]
            self.diagnostics[params["uri"]] = params["diagnostics"]
        return message

    def request(self, method, params):
        request_id = self.next_id
        self.next_id += 1
        self._send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        while True:
            message = self._pump()
            if message.get("id") == request_id:
                if "error" in message:
                    raise RuntimeError(message["error"])
                return message.get("result")

    def notify(self, method, params):
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def initialize(self):
        result = self.request("initialize", {"processId": None, "rootUri": None, "capabilities": {}})
        self.notify("initialized", {})
        return result

    def _invalidate(self, path):
        self.diagnostics.pop(Path(path).as_uri(), None)

    def did_open(self, path, text, version=1):
        self._invalidate(path)
        self.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": Path(path).as_uri(),
                    "languageId": "java",
                    "version": version,
                    "text": text,
                }
            },
        )

    def did_change(self, path, text, version=2):
        self._invalidate(path)
        self.notify(
            "textDocument/didChange",
            {
                "textDocument": {"uri": Path(path).as_uri(), "version": version},
                "contentChanges": [{"text": text}],
            },
        )

    def did_save(self, path, text=None):
        self._invalidate(path)
        params = {"textDocument": {"uri": Path(path).as_uri()}}
        if text is not None:
            params["text"] = text
        self.notify("textDocument/didSave", params)

    def did_close(self, path):
        self._invalidate(path)
        self.notify("textDocument/didClose", {"textDocument": {"uri": Path(path).as_uri()}})

    def wait_diagnostics(self, path, timeout=15.0):
        uri = Path(path).as_uri()
        deadline = time.time() + timeout
        while time.time() < deadline:
            if uri in self.diagnostics:
                return self.diagnostics[uri]
            message = self._read_message()
            if message is None:
                raise RuntimeError("server closed the connection")
            if message.get("method") == "textDocument/publishDiagnostics":
                params = message["params"]
                self.diagnostics[params["uri"]] = params["diagnostics"]
        raise RuntimeError("no diagnostics for " + uri)

    def completion(self, path, line, character):
        return self.request(
            "textDocument/completion",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
            },
        ) or []

    def definition(self, path, line, character):
        return self.request(
            "textDocument/definition",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
            },
        )

    def hover(self, path, line, character):
        return self.request(
            "textDocument/hover",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
            },
        )

    def references(self, path, line, character):
        return self.request(
            "textDocument/references",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
                "context": {"includeDeclaration": True},
            },
        ) or []

    def prepare_call_hierarchy(self, path, line, character):
        return self.request(
            "textDocument/prepareCallHierarchy",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
            },
        )

    def incoming_calls(self, item):
        return self.request("callHierarchy/incomingCalls", {"item": item}) or []

    def outgoing_calls(self, item):
        return self.request("callHierarchy/outgoingCalls", {"item": item}) or []

    def prepare_type_hierarchy(self, path, line, character):
        return self.request(
            "textDocument/prepareTypeHierarchy",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
            },
        )

    def type_hierarchy_subtypes(self, item):
        return self.request("typeHierarchy/subtypes", {"item": item}) or []

    def type_hierarchy_supertypes(self, item):
        return self.request("typeHierarchy/supertypes", {"item": item}) or []

    def code_action(self, path, line, character):
        point = {"line": line, "character": character}
        return self.request(
            "textDocument/codeAction",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "range": {"start": point, "end": point},
                "context": {"diagnostics": []},
            },
        ) or []

    def signature_help(self, path, line, character):
        return self.request(
            "textDocument/signatureHelp",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
            },
        )

    def implementation(self, path, line, character):
        return self.request(
            "textDocument/implementation",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
            },
        ) or []

    def type_definition(self, path, line, character):
        return self.request(
            "textDocument/typeDefinition",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
            },
        )

    def rename(self, path, line, character, new_name):
        return self.request(
            "textDocument/rename",
            {
                "textDocument": {"uri": Path(path).as_uri()},
                "position": {"line": line, "character": character},
                "newName": new_name,
            },
        )

    def document_symbols(self, path):
        return self.request(
            "textDocument/documentSymbol",
            {"textDocument": {"uri": Path(path).as_uri()}},
        ) or []

    def shutdown(self):
        self.request("shutdown", {})
        self.notify("exit", {})
        self.proc.wait(timeout=10)
        return self.proc.returncode
