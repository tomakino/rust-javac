#!/usr/bin/env python3
"""C: LSP 3.17 protocol conformance audit.

Covers position encoding negotiation, URI encoding, document symbol shape,
hover Markdown, empty results, out-of-range requests, full-file ranges,
$/cancelRequest and shutdown/exit.

Usage:
    py scripts/lsp/probe_c_protocol.py
"""

import json
import os
import queue
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path
from urllib.parse import unquote, urlparse
from urllib.request import url2pathname

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
BASE = ROOT / "target" / "tmp" / "c-protocol" / ("run-" + str(os.getpid()))

POM = (
    "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId>"
    "<artifactId>proto</artifactId><version>1.0</version>"
    "<properties><maven.compiler.release>17</maven.compiler.release></properties></project>\n"
)
HELPER = """package com.example.base;

public class Helper {
    public int value;

    public void compute() {
    }
}
"""
APP = """package com.example.app;

import com.example.base.Helper;

import java.util.List;

public class App {
    Helper helper = new Helper();

    List<String> names;

    void f() {
        helper.compute();
    }
}
"""
EMPTY = "package com.example.app;\n\npublic class Empty {\n}\n"
APP_PATH = "src/main/java/com/example/app/App.java"
HELPER_PATH = "src/main/java/com/example/base/Helper.java"


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def position_of(text: str, marker: str, offset: int = 0):
    start = text.index(marker) + offset
    prefix = text[:start]
    return prefix.count("\n"), start - (prefix.rfind("\n") + 1)


class Lsp:
    def __init__(self, root: Path, extra_init: dict | None = None):
        self.root = root
        self.proc = subprocess.Popen(
            [str(EXE)], stdin=subprocess.PIPE, stdout=subprocess.PIPE
        )
        self.messages: "queue.Queue[dict]" = queue.Queue()
        threading.Thread(target=self._reader, daemon=True).start()
        self.next_id = 100
        params = {"rootUri": root.as_uri()}
        if extra_init:
            params.update(extra_init)
        self.init_result = self.request("initialize", params, request_id=1)

    def _reader(self):
        while True:
            headers = b""
            while b"\r\n\r\n" not in headers:
                chunk = self.proc.stdout.read(1)
                if not chunk:
                    return
                headers += chunk
            length = int(headers.split(b"Content-Length: ")[1].split(b"\r\n")[0])
            self.messages.put(json.loads(self.proc.stdout.read(length)))

    def send(self, message):
        body = json.dumps(message).encode()
        self.proc.stdin.write(
            b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body
        )
        self.proc.stdin.flush()

    def read_until(self, request_id, timeout: float = 20.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                message = self.messages.get(timeout=max(0.0, deadline - time.time()))
            except queue.Empty:
                return None
            if message.get("id") == request_id:
                return message
        return None

    def request(self, method, params, request_id=None):
        if request_id is None:
            self.next_id += 1
            request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        return (self.read_until(request_id) or {}).get("result")

    def uri(self, relative: str) -> str:
        return (self.root / relative).as_uri()

    def did_open(self, relative: str, text: str):
        self.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": self.uri(relative),
                        "languageId": "java",
                        "version": 1,
                        "text": text,
                    }
                },
            }
        )

    def definition(self, relative, position):
        return self.request(
            "textDocument/definition",
            {
                "textDocument": {"uri": self.uri(relative)},
                "position": {"line": position[0], "character": position[1]},
            },
        )

    def hover(self, relative, position):
        return self.request(
            "textDocument/hover",
            {
                "textDocument": {"uri": self.uri(relative)},
                "position": {"line": position[0], "character": position[1]},
            },
        )

    def symbols(self, relative):
        return self.request(
            "textDocument/documentSymbol", {"textDocument": {"uri": self.uri(relative)}}
        )

    def links(self, relative):
        return self.request(
            "textDocument/documentLink", {"textDocument": {"uri": self.uri(relative)}}
        )

    def close(self):
        try:
            self.request("shutdown", {}, request_id=9999)
            self.send({"jsonrpc": "2.0", "method": "exit", "params": {}})
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


def project(name: str) -> Path:
    root = BASE / name / "project"
    shutil.rmtree(root.parent, ignore_errors=True)
    write(root / "pom.xml", POM)
    write(root / HELPER_PATH, HELPER)
    write(root / APP_PATH, APP)
    write(root / "src/main/java/com/example/app/Empty.java", EMPTY)
    return root


def rel_of(location) -> str:
    if not location:
        return ""
    return unquote(url2pathname(urlparse(location.get("uri", "")).path)).replace("\\", "/")


def contains_range(outer, inner) -> bool:
    def key(pos):
        return (pos["line"], pos["character"])

    return key(outer["start"]) <= key(inner["start"]) and key(inner["end"]) <= key(outer["end"])


def main():
    rows = []

    def row(case_id, title, ok, actual):
        rows.append({"id": case_id, "title": title, "ok": ok, "actual": actual})

    # 1. positionEncoding negotiation: default UTF-16, explicit utf-8 declarable
    root = project("1-encoding")
    default = Lsp(root)
    try:
        capabilities = (default.init_result or {}).get("capabilities") or {}
        declared_default = capabilities.get("positionEncoding")
        explicit = Lsp(root, {"general": {"positionEncoding": "utf-8"}})
        try:
            capabilities_utf8 = (explicit.init_result or {}).get("capabilities") or {}
            declared_utf8 = capabilities_utf8.get("positionEncoding")
        finally:
            explicit.close()
        # LSP 3.17: absent means the client must use UTF-16. The server does
        # not announce utf-8, so even an explicit client request stays UTF-16.
        ok = declared_default is None and declared_utf8 in (None, "utf-16")
        row("1-position-encoding", "positionEncoding 协商（默认 UTF-16；utf-8 请求被忽略）", ok,
            f"default_declared={declared_default} utf8_declared={declared_utf8} (absent => UTF-16)")
    finally:
        default.close()

    # 2. URI encoding for Chinese/space/#/% path and file name
    root = BASE / "2-uri" / "中文 目录" / "project"
    shutil.rmtree(BASE / "2-uri", ignore_errors=True)
    write(root / "pom.xml", POM)
    special_rel = "src/main/java/com/example/special/a#b%.java"
    write(
        root / special_rel,
        "package com.example.special;\n\npublic class Special {\n    public int v;\n}\n",
    )
    app = "package com.example.app;\n\nimport com.example.special.Special;\n\npublic class App {\n    Special special = new Special();\n}\n"
    write(root / APP_PATH, app)
    lsp = Lsp(root)
    try:
        lsp.did_open(APP_PATH, app)
        pos = position_of(app, "new Special", len("new "))
        location = lsp.definition(APP_PATH, pos)
        uri = (location or {}).get("uri", "")
        raw_ok = uri != "" and "#" not in uri and " " not in uri
        decoded = rel_of(location)
        links = lsp.links(APP_PATH) or []
        link_uris = [link.get("target", "") for link in links]
        links_ok = all("#" not in target and " " not in target for target in link_uris)
        ok = raw_ok and decoded.endswith("a#b%.java") and links_ok
        row("2-uri-encoding", "中文/空格/#/% 路径的 URI 百分号编码可解析", ok,
            f"def_uri={uri[-60:]} decoded={decoded[-40:]} links={len(link_uris)}")
    finally:
        lsp.close()

    # 3/4/5/6/7/8 share one server
    root = project("3-shape")
    lsp = Lsp(root)
    try:
        lsp.did_open(APP_PATH, APP)
        symbols = lsp.symbols(APP_PATH) or []

        def shape_ok(symbol):
            name = symbol.get("name") or ""
            kind = symbol.get("kind")
            rng = symbol.get("range")
            selection = symbol.get("selectionRange")
            if not name or not isinstance(kind, int) or not (1 <= kind <= 26):
                return False
            if not rng or not selection:
                return False
            return contains_range(rng, selection)

        bad = [symbol.get("name") for symbol in symbols if not shape_ok(symbol)]
        empty_symbols = lsp.symbols("src/main/java/com/example/app/Empty.java") or []
        ok = len(symbols) >= 1 and not bad and len(empty_symbols) >= 1
        row("3-document-symbol", "DocumentSymbol：name/kind/selectionRange ⊆ range 合法", ok,
            f"symbols={len(symbols)} bad={bad[:3]} empty_file_symbols={len(empty_symbols)}")

        # 4. hover Markdown
        pos = position_of(APP, "helper", 0)
        hover = lsp.hover(APP_PATH, pos)
        value = ((hover or {}).get("contents") or {}).get("value", "")
        fences = value.count("```")
        control = any(ord(ch) < 32 and ch not in "\n\t" for ch in value)
        ok = isinstance(value, str) and value != "" and fences % 2 == 0 and not control
        row("4-hover-markdown", "hover Markdown：```java 闭合、无控制字符", ok,
            f"fences={fences} control={control} head={value[:40]!r}")

        # 5. empty response semantics
        missing = lsp.definition(APP_PATH, (0, 0))
        empty_def = lsp.definition("src/main/java/com/example/app/Empty.java", (2, 7))
        ok = missing is None and empty_def is None
        row("5-empty-response", "找不到时 definition 返回 null（非 {}）", ok,
            f"missing={json.dumps(missing)} empty={json.dumps(empty_def)}")

        # 6. error responses: missing file / out-of-range positions
        missing_uri = (root / "src/main/java/com/example/app/Missing.java").as_uri()
        missing_file_def = lsp.request(
            "textDocument/definition",
            {"textDocument": {"uri": missing_uri}, "position": {"line": 0, "character": 0}},
        )
        missing_file_hover = lsp.request(
            "textDocument/hover",
            {"textDocument": {"uri": missing_uri}, "position": {"line": 0, "character": 0}},
        )
        far = lsp.definition(APP_PATH, (9999, 0))
        far_hover = lsp.hover(APP_PATH, (9999, 0))
        after = lsp.definition(APP_PATH, position_of(APP, "new Helper", len("new ")))
        ok = missing_file_def is None and missing_file_hover is None and far is None and far_hover is None and after is not None
        row("6-error-response", "不存在文件/位置越界 → null，不崩溃且后续正常", ok,
            f"missing_file={missing_file_def is None} far_def={far is None} far_hover={far_hover is None} after_ok={after is not None}")

        # 7. full-file range endpoints
        lines = APP.split("\n")
        last = (len(lines) - 1, len(lines[-1]))
        start_hover = lsp.hover(APP_PATH, (0, 0))
        end_hover = lsp.hover(APP_PATH, last)
        end_def = lsp.definition(APP_PATH, last)

        def valid(value):
            if value is None:
                return True
            if isinstance(value, dict) and "range" in value:
                rng = value["range"]
                return rng["start"]["line"] <= last[0] and rng["end"]["line"] <= last[0]
            return False

        ok = valid(start_hover) and valid(end_hover) and valid(end_def)
        row("7-full-range", "全文件 range (0,0)-(last,last) 不越界", ok,
            f"end={last} hover_end={end_hover is not None} def_end={end_def is not None}")

        # 8. $/cancelRequest
        lsp.send({"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 424242}})
        alive = lsp.definition(APP_PATH, position_of(APP, "new Helper", len("new ")))
        ok = alive is not None
        row("8-cancel-request", "$/cancelRequest 后不崩溃、后续请求正常", ok,
            f"alive={alive is not None}")
    finally:
        lsp.close()

    # 9. shutdown/exit
    root = project("4-shutdown")
    lsp = Lsp(root)
    try:
        lsp.did_open(APP_PATH, APP)
        lsp.send({"jsonrpc": "2.0", "id": 700, "method": "shutdown", "params": {}})
        shutdown = lsp.read_until(700)
        shutdown_ok = shutdown is not None and shutdown.get("result") is None
        # Requests after shutdown must not hang or crash; the server may answer
        # or stay silent (LSP forbids sending them, so either is tolerable).
        lsp.send(
            {
                "jsonrpc": "2.0",
                "id": 701,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": {"uri": lsp.uri(APP_PATH)},
                    "position": {"line": 3, "character": 7},
                },
            }
        )
        post = lsp.read_until(701, timeout=3)
        lsp.send({"jsonrpc": "2.0", "method": "exit", "params": {}})
        try:
            lsp.proc.wait(timeout=5)
            exited = True
        except subprocess.TimeoutExpired:
            exited = False
            lsp.proc.kill()
        ok = shutdown_ok and exited and (post is None or "result" in post)
        row("9-shutdown-exit", "shutdown 响应 null；exit 后进程退出", ok,
            f"shutdown_ok={shutdown_ok} post_shutdown={'silent' if post is None else 'answered'} exited={exited}")
    finally:
        if lsp.proc.poll() is None:
            lsp.proc.kill()

    failed = [entry for entry in rows if not entry["ok"]]
    for entry in rows:
        print(("PASS  " if entry["ok"] else "FAIL  ") + entry["id"] + "  " + entry["title"])
        print("      " + entry["actual"])
    print("")
    print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " cases passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
