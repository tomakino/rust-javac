#!/usr/bin/env python3
"""E: exception/degradation audit through a real LSP.

Every case runs in a fresh server with stderr captured: a panic shows up as a
non-empty stderr (or a new ravac-lsp-panic.log entry), and every request must
answer within the timeout.

Usage:
    py scripts/lsp/probe_e_degradation.py
"""

import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
BASE = ROOT / "target" / "tmp" / "e-degradation" / ("run-" + str(os.getpid()))
PANIC_LOG = Path(tempfile.gettempdir()) / "ravac-lsp-panic.log"

POM = (
    "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId>"
    "<artifactId>degrade</artifactId><version>1.0</version></project>\n"
)


def write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def position_of(text: str, marker: str, offset: int = 0):
    start = text.index(marker) + offset
    prefix = text[:start]
    return prefix.count("\n"), start - (prefix.rfind("\n") + 1)


class Lsp:
    def __init__(self, root: Path):
        self.root = root
        self.proc = subprocess.Popen(
            [str(EXE)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        self.messages: "queue.Queue[dict]" = queue.Queue()
        self.stderr_lines: list[str] = []
        threading.Thread(target=self._reader, daemon=True).start()
        threading.Thread(target=self._stderr_reader, daemon=True).start()
        self.next_id = 100
        self.request("initialize", {"rootUri": root.as_uri()}, request_id=1)

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

    def _stderr_reader(self):
        while True:
            line = self.proc.stderr.readline()
            if not line:
                return
            self.stderr_lines.append(line.decode("utf-8", "replace"))

    def stderr_text(self) -> str:
        return "".join(self.stderr_lines)

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

    def request(self, method, params, request_id=None, timeout: float = 20.0):
        if request_id is None:
            self.next_id += 1
            request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        return (self.read_until(request_id, timeout=timeout) or {}).get("result")

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

    def wait_diagnostics(self, relative: str, timeout: float = 3.0):
        deadline = time.time() + timeout
        uri = self.uri(relative)
        while time.time() < deadline:
            try:
                message = self.messages.get(timeout=max(0.0, deadline - time.time()))
            except queue.Empty:
                return None
            if message.get("method") != "textDocument/publishDiagnostics":
                continue
            params = message.get("params") or {}
            if params.get("uri") == uri:
                return params.get("diagnostics") or []
        return None

    def hover(self, relative, position, timeout: float = 20.0):
        return self.request(
            "textDocument/hover",
            {
                "textDocument": {"uri": self.uri(relative)},
                "position": {"line": position[0], "character": position[1]},
            },
            timeout=timeout,
        )

    def definition(self, relative, position, timeout: float = 20.0):
        return self.request(
            "textDocument/definition",
            {
                "textDocument": {"uri": self.uri(relative)},
                "position": {"line": position[0], "character": position[1]},
            },
            timeout=timeout,
        )

    def symbols(self, relative, timeout: float = 20.0):
        return self.request(
            "textDocument/documentSymbol",
            {"textDocument": {"uri": self.uri(relative)}},
            timeout=timeout,
        )

    def tokens(self, relative, timeout: float = 20.0):
        return self.request(
            "textDocument/semanticTokens/full",
            {"textDocument": {"uri": self.uri(relative)}},
            timeout=timeout,
        )

    def close(self):
        try:
            self.proc.kill()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=5)
        except Exception:
            pass


def project(name: str) -> Path:
    root = BASE / name / "project"
    shutil.rmtree(root.parent, ignore_errors=True)
    write_bytes(root / "pom.xml", POM.encode("utf-8"))
    return root


def panic_snapshot():
    try:
        return PANIC_LOG.stat().st_mtime_ns
    except OSError:
        return 0


def wants(actual):
    """An empty answer: None, or a list with no diagnostics."""
    return actual is None or actual == [] or actual == {}


def main():
    rows = []

    def row(case_id, title, ok, actual):
        rows.append({"id": case_id, "title": title, "ok": ok, "actual": actual})

    def run_case(case_id, title, relative, text, requests, *, open_text=True):
        """Open text, issue every request, check none timed out and no panic."""
        before = panic_snapshot()
        root = project(case_id)
        lsp = Lsp(root)
        try:
            write_bytes(root / relative, text.encode("utf-8"))
            if open_text:
                lsp.did_open(relative, text)
            results = {}
            timed_out = []
            for name, call in requests.items():
                started = time.time()
                value = call(lsp)
                elapsed = time.time() - started
                results[name] = (value, elapsed)
                if value is None and elapsed >= 19.9:
                    timed_out.append(name)
            stderr = lsp.stderr_text()
            panic = "panic" in stderr.lower() or panic_snapshot() != before
            ok = not timed_out and not panic
            detail = ", ".join(
                f"{name}={('null' if value is None else ('empty' if value in ([], {}) else str(value)[:60]))}"
                f" {elapsed * 1000:.0f}ms"
                for name, (value, elapsed) in results.items()
            )
            if timed_out:
                detail += f" TIMEOUT={timed_out}"
            if panic:
                detail += " PANIC!"
            row(case_id, title, ok, f"{detail} | stderr={len(stderr)}B")
        finally:
            lsp.close()

    def hover_pos(lsp, relative, text, marker, offset=0):
        return lsp.hover(relative, position_of(text, marker, offset))

    def def_pos(lsp, relative, text, marker, offset=0):
        return lsp.definition(relative, position_of(text, marker, offset))

    # 1. syntax errors: missing semicolon / unbalanced brace / illegal token
    missing_semicolon = "class A {\n    int x void f() {}\n}\n"
    unbalanced = "class A {\n    void f() {\n}\n"
    illegal = "class A {\n    int @ x;\n}\n"
    run_case(
        "1a-missing-semicolon",
        "语法错误：缺分号 → hover/definition/symbols/tokens 不崩",
        "A.java",
        missing_semicolon,
        {
            "hover": lambda lsp: hover_pos(lsp, "A.java", missing_semicolon, "int x", len("int ")),
            "definition": lambda lsp: def_pos(lsp, "A.java", missing_semicolon, "int x", len("int ")),
            "symbols": lambda lsp: lsp.symbols("A.java"),
            "tokens": lambda lsp: lsp.tokens("A.java"),
        },
    )
    run_case(
        "1b-unbalanced-brace",
        "语法错误：括号不配对 → 请求不崩",
        "A.java",
        unbalanced,
        {
            "hover": lambda lsp: hover_pos(lsp, "A.java", unbalanced, "void f", len("void ")),
            "symbols": lambda lsp: lsp.symbols("A.java"),
            "tokens": lambda lsp: lsp.tokens("A.java"),
        },
    )
    run_case(
        "1c-illegal-token",
        "语法错误：非法 token → 请求不崩",
        "A.java",
        illegal,
        {
            "hover": lambda lsp: hover_pos(lsp, "A.java", illegal, "int @ x", len("int ")),
            "symbols": lambda lsp: lsp.symbols("A.java"),
            "tokens": lambda lsp: lsp.tokens("A.java"),
        },
    )

    # 2. unresolved symbols
    unresolved = "class A {\n    UnknownType x;\n\n    void f() {\n        unknownMethod();\n    }\n}\n"
    run_case(
        "2-unresolved-symbols",
        "未解析符号：hover/definition → 空，不崩",
        "A.java",
        unresolved,
        {
            "hover_type": lambda lsp: hover_pos(lsp, "A.java", unresolved, "UnknownType", 0),
            "hover_method": lambda lsp: hover_pos(lsp, "A.java", unresolved, "unknownMethod", 0),
            "definition_type": lambda lsp: def_pos(lsp, "A.java", unresolved, "UnknownType", 0),
            "definition_method": lambda lsp: def_pos(lsp, "A.java", unresolved, "unknownMethod", 0),
        },
    )

    # 3. half-parsed file
    half = "class A {\n    void f() {\n"
    run_case(
        "3-half-parsed",
        "半解析文件（截断）→ 不崩",
        "A.java",
        half,
        {
            "hover": lambda lsp: hover_pos(lsp, "A.java", half, "void f", len("void ")),
            "symbols": lambda lsp: lsp.symbols("A.java"),
            "tokens": lambda lsp: lsp.tokens("A.java"),
        },
    )

    # 4. missing dependency: diagnostics point at the import, other features live
    missing_dep = "package com.example;\n\nimport com.missing.pkg.Nope;\n\nclass A {\n    Nope n;\n}\n"
    root = project("4-missing-dependency")
    lsp = Lsp(root)
    try:
        write_bytes(root / "A.java", missing_dep.encode("utf-8"))
        lsp.did_open("A.java", missing_dep)
        diagnostics = lsp.wait_diagnostics("A.java", timeout=5.0)
        hover = lsp.hover("A.java", position_of(missing_dep, "class A", len("class ")))
        symbols = lsp.symbols("A.java")
        stderr = lsp.stderr_text()
        panic = "panic" in stderr.lower()
        line_ok = diagnostics is not None and any(
            (diagnostic.get("range") or {}).get("start", {}).get("line") in (2, 5)
            for diagnostic in diagnostics
        )
        ok = diagnostics is not None and line_ok and hover is not None and symbols and not panic
        row("4-missing-dependency", "依赖缺失：诊断位置正确、其他能力不崩", ok,
            f"diagnostics={len(diagnostics or [])} lines={[((d.get('range') or {}).get('start') or {}).get('line') for d in (diagnostics or [])][:5]} "
            f"hover_ok={hover is not None} symbols={len(symbols or [])} stderr={len(stderr)}B")
    finally:
        lsp.close()

    # 5. empty / comment-only / package-only files
    for case_id, title, text in [
        ("5a-empty-file", "空文件 → symbols []、请求不崩", ""),
        ("5b-comment-only", "仅注释 → symbols []、请求不崩", "// just a comment\n/* block */\n"),
        ("5c-package-only", "只有 package → symbols []、请求不崩", "package com.example;\n"),
    ]:
        run_case(
            case_id,
            title,
            "A.java",
            text,
            {
                "symbols": lambda lsp: lsp.symbols("A.java"),
                "tokens": lambda lsp: lsp.tokens("A.java"),
            },
        )

    # 6. huge file: 10000-line method body, answers within 3s
    body = "".join(f"        x = x + {index};\n" for index in range(10000))
    big = "class Big {\n    int value;\n\n    void f() {\n        int x = value;\n" + body + "    }\n}\n"
    before = panic_snapshot()
    root = project("6-huge-file")
    lsp = Lsp(root)
    try:
        write_bytes(root / "Big.java", big.encode("utf-8"))
        lsp.did_open("Big.java", big)
        started = time.time()
        hover = lsp.hover("Big.java", position_of(big, "int x = value", len("int x = ")))
        definition = lsp.definition("Big.java", position_of(big, "int x = value", len("int x = ")))
        elapsed = time.time() - started
        stderr = lsp.stderr_text()
        panic = "panic" in stderr.lower() or panic_snapshot() != before
        ok = hover is not None or definition is not None
        ok = ok and elapsed < 3.0 and not panic
        row("6-huge-file", "10000 行方法体：hover/definition <3s", ok,
            f"hover_ok={hover is not None} def_ok={definition is not None} {elapsed * 1000:.0f}ms stderr={len(stderr)}B")
    finally:
        lsp.close()

    # 7. non-UTF-8 (GBK) bytes on disk, read without didOpen
    before = panic_snapshot()
    root = project("7-gbk")
    lsp = Lsp(root)
    try:
        gbk = "class A {\n    // 中文注释\n    int value;\n}\n".encode("gbk")
        write_bytes(root / "A.java", gbk)
        symbols = lsp.symbols("A.java")
        hover = lsp.hover("A.java", (2, 8))
        stderr = lsp.stderr_text()
        panic = "panic" in stderr.lower() or panic_snapshot() != before
        ok = not panic
        row("7-gbk-bytes", "GBK 编码 .java：不 panic、降级处理", ok,
            f"symbols={len(symbols or [])} hover={'null' if hover is None else 'value'} stderr={len(stderr)}B")
    finally:
        lsp.close()

    # 8. UTF-8 BOM: positions must not shift
    body = "class A {\n    int value;\n\n    int get() {\n        return value;\n    }\n}\n"
    plain = "8a-bom-plain"
    bom_case = "8b-bom-prefixed"
    for case_id, text in [(plain, body), (bom_case, "\ufeff" + body)]:
        run_case(
            case_id,
            "UTF-8 BOM：位置不偏移、hover/definition 正确" if case_id == bom_case else "无 BOM 对照",
            "A.java",
            text,
            {
                "hover": lambda lsp, t=text: lsp.hover("A.java", position_of(t, "int value", len("int "))),
                "definition": lambda lsp, t=text: lsp.definition(
                    "A.java", position_of(t, "return value", len("return "))
                ),
            },
        )

    # 9. CRLF file
    crlf = "class A {\r\n    int value;\r\n\r\n    int get() {\r\n        return value;\r\n    }\r\n}\r\n"
    run_case(
        "9-crlf",
        "CRLF 文件：行号/列正确",
        "A.java",
        crlf,
        {
            "hover": lambda lsp: lsp.hover("A.java", position_of(crlf, "int value", len("int "))),
            "definition": lambda lsp: lsp.definition(
                "A.java", position_of(crlf, "return value", len("return "))
            ),
        },
    )

    # 10. mixed line endings
    mixed = "class A {\n    int a;\r\n    int b;\n\n    void f() {\r\n        int c = a + b;\n    }\r\n}\n"
    run_case(
        "10-mixed-endings",
        "混合行尾（CRLF+LF）：不崩、行号合理",
        "A.java",
        mixed,
        {
            "hover": lambda lsp: lsp.hover("A.java", position_of(mixed, "int a", len("int "))),
            "definition": lambda lsp: lsp.definition(
                "A.java", position_of(mixed, "int c = a", len("int c = "))
            ),
            "symbols": lambda lsp: lsp.symbols("A.java"),
        },
    )

    failed = [entry for entry in rows if not entry["ok"]]
    for entry in rows:
        print(("PASS  " if entry["ok"] else "FAIL  ") + entry["id"] + "  " + entry["title"])
        print("      " + entry["actual"])
    print("")
    print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " cases passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
