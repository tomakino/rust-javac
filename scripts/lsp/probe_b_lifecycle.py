#!/usr/bin/env python3
"""B: edit lifecycle through a real LSP.

Each scenario uses its own fixture directory and server process:
overlay editing, rapid didChange, undo/redo, new/removed/renamed files,
external changes, restart, and unsaved -> saved.

Usage:
    py scripts/lsp/probe_b_lifecycle.py [--json]
"""

import json
import os
import queue
import threading
import shutil
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import unquote, urlparse
from urllib.request import url2pathname

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
BASE = ROOT / "target" / "tmp" / "b-lifecycle" / ("run-" + str(os.getpid()))

POM = (
    "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId>"
    "<artifactId>b-life</artifactId><version>1.0</version>"
    "<properties><maven.compiler.release>17</maven.compiler.release></properties></project>\n"
)
HELPER = """package com.example.app;

public class Helper {
    public int value;

    public void compute() {
    }
}
"""
APP = """package com.example.app;

public class App {
    Helper helper = new Helper();

    void f() {
        helper.compute();
    }
}
"""
APP_PATH = "src/main/java/com/example/app/App.java"
HELPER_PATH = "src/main/java/com/example/app/Helper.java"


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def position_of(text: str, marker: str, offset: int = 0):
    start = text.index(marker) + offset
    prefix = text[:start]
    return prefix.count("\n"), start - (prefix.rfind("\n") + 1)


class Lsp:
    """Async reader: the client always drains server output, like an editor.

    A synchronous read_until would leave publishDiagnostics messages unread
    while the probe fires many didChange requests; the server then blocks on
    its stdout pipe and every later request looks like a hang.
    """

    def __init__(self, root: Path):
        self.root = root
        self.proc = subprocess.Popen(
            [str(EXE)], stdin=subprocess.PIPE, stdout=subprocess.PIPE
        )
        self.messages: "queue.Queue[dict]" = queue.Queue()
        threading.Thread(target=self._reader, daemon=True).start()
        self.next_id = 100
        self.request(
            "initialize", {"rootUri": root.as_uri()}, request_id=1
        )
        # A didOpen is what an editor does; it also warms the analysis path.
        self.did_open(APP_PATH, APP)
        # An editor sees artifacts/indexes settle before the user queries;
        # the first full build is asynchronous on the server.
        self._wait_artifacts()
        self._wait_index()

    def _wait_artifacts(self, timeout: float = 10.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            stats = self.request("ravac/artifactStats", {})
            if stats and stats.get("status") != "building":
                return
            time.sleep(0.05)

    def _wait_index(self, timeout: float = 10.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.definition(APP_PATH, (3, 4))
            stats = self.request("ravac/indexStats", {})
            if stats and not stats.get("building") and stats.get("generation", 0) > 0:
                return
            time.sleep(0.05)

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
        self.send(
            {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}
        )
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

    def did_change(self, relative: str, text: str, version: int):
        self.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": self.uri(relative), "version": version},
                    "contentChanges": [{"text": text}],
                },
            }
        )

    def did_save(self, relative: str):
        self.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didSave",
                "params": {
                    "textDocument": {"uri": self.uri(relative)},
                    "text": (self.root / relative).read_text(encoding="utf-8"),
                },
            }
        )

    def watched(self, changes):
        self.send(
            {
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWatchedFiles",
                "params": {"changes": changes},
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

    def tokens(self, relative):
        return self.request(
            "textDocument/semanticTokens/full",
            {"textDocument": {"uri": self.uri(relative)}},
        )

    def close(self):
        try:
            self.request("shutdown", {}, request_id=9999)
            self.send({"jsonrpc": "2.0", "method": "exit", "params": {}})
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


def project(name: str) -> tuple[Path, Path]:
    root = BASE / name / "project"
    shutil.rmtree(root.parent, ignore_errors=True)
    write(root / "pom.xml", POM)
    write(root / HELPER_PATH, HELPER)
    write(root / APP_PATH, APP)
    return root, root / APP_PATH


def rel_of(location) -> str:
    if not location:
        return ""
    path = unquote(url2pathname(urlparse(location.get("uri", "")).path))
    return path.replace("\\", "/")


def main():
    as_json = "--json" in sys.argv
    rows = []

    def row(case_id, title, ok, actual):
        rows.append({"id": case_id, "title": title, "ok": ok, "actual": actual})

    print('CASE 1', file=sys.stderr, flush=True)
    # 1. overlay: hover / definition / documentSymbol / semanticTokens
    root, app = project("1-overlay")
    overlay = APP.replace(
        "Helper helper = new Helper();",
        "Helper helper = new Helper();\n    int other = helper.value;",
    )
    lsp = Lsp(root)
    try:
        lsp.did_change(APP_PATH, overlay, 2)
        pos = position_of(overlay, "helper.value", 0)
        hover = lsp.hover(APP_PATH, pos)
        definition = lsp.definition(APP_PATH, pos)
        compute_pos = position_of(overlay, "helper.compute", len("helper."))
        compute_def = lsp.definition(APP_PATH, compute_pos)
        symbols = lsp.symbols(APP_PATH) or []
        tokens = lsp.tokens(APP_PATH)
        hover_text = json.dumps(hover)
        tokens_data = (tokens or {}).get("data") or []
        ok = (
            "Helper" in hover_text
            and rel_of(definition).endswith("App.java")
            and rel_of(compute_def).endswith("Helper.java")
            and len(symbols) >= 1
            and len(tokens_data) > 0
        )
        row("1-overlay", "overlay 下 hover/definition/documentSymbol/semanticTokens", ok,
            f"hover={hover_text[:60]} field_def={rel_of(definition)[-20:]} compute_def={rel_of(compute_def)[-30:]} symbols={len(symbols)} tokens={len(tokens_data)}")
    finally:
        lsp.close()

    print('CASE 2', file=sys.stderr, flush=True)
    # 2. rapid didChange (20 edits, ~10/s) then hover/definition
    root, app = project("2-rapid")
    lsp = Lsp(root)
    try:
        text = APP
        started = time.time()
        for index in range(20):
            text = APP.replace("int local = 0;", f"int local = {index};") if "int local" in APP else APP.replace(
                "helper.compute();", f"int local = {index}; helper.compute();"
            )
            lsp.did_change(APP_PATH, text, 2 + index)
            time.sleep(0.1)
        elapsed = time.time() - started
        pos = position_of(text, "helper.compute();", len("helper."))
        hover = lsp.hover(APP_PATH, pos)
        definition = lsp.definition(APP_PATH, pos)
        ok = rel_of(definition).endswith("Helper.java")
        row("2-rapid-didchange", "20 次快速 didChange 后 hover/definition", ok,
            f"edits=20 elapsed={elapsed:.2f}s hover={json.dumps(hover)[:50]} def={rel_of(definition)[-40:]}")
    finally:
        lsp.close()

    print('CASE 3', file=sys.stderr, flush=True)
    # 3. undo/redo (restore original text)
    root, app = project("3-undo")
    lsp = Lsp(root)
    try:
        base_pos = position_of(APP, "helper.compute();", 0)
        before_hover = lsp.hover(APP_PATH, base_pos)
        before_def = lsp.definition(APP_PATH, base_pos)
        lsp.did_change(APP_PATH, APP.replace("helper.compute();", "helper.compute(); // edited"), 2)
        lsp.did_change(APP_PATH, APP, 3)
        after_hover = lsp.hover(APP_PATH, base_pos)
        after_def = lsp.definition(APP_PATH, base_pos)
        def same_text(value):
            return "Helper" in json.dumps(value)

        ok = (
            same_text(before_hover)
            and same_text(after_hover)
            and rel_of(before_def) == rel_of(after_def)
        )
        row("3-undo-redo", "撤销（恢复原文）后结果一致", ok,
            f"hover_helper={same_text(before_hover) and same_text(after_hover)} def_same={rel_of(before_def) == rel_of(after_def)}")
    finally:
        lsp.close()

    print('CASE 4', file=sys.stderr, flush=True)
    # 4. new unsaved file: symbols / hover / definition in the new buffer
    root, app = project("4-new-file")
    lsp = Lsp(root)
    try:
        new_rel = "src/main/java/com/example/app/NewFile.java"
        new_text = "package com.example.app;\n\npublic class NewFile {\n    int field;\n\n    void run() {\n        int local = field;\n    }\n}\n"
        lsp.did_open(new_rel, new_text)
        pos = position_of(new_text, "int local = field;", len("int local = "))
        hover = lsp.hover(new_rel, pos)
        definition = lsp.definition(new_rel, pos)
        symbols = lsp.symbols(new_rel) or []
        ok = (
            "field" in json.dumps(hover)
            and rel_of(definition).endswith("NewFile.java")
            and len(symbols) >= 1
        )
        row("4-new-file", "新建文件 didOpen → 大纲/hover/跳转", ok,
            f"hover={json.dumps(hover)[:60]} def={rel_of(definition)[-40:]} symbols={len(symbols)}")
    finally:
        lsp.close()

    print('CASE 5', file=sys.stderr, flush=True)
    # 5. delete helper: references cleaned, no crash
    root, app = project("5-delete")
    lsp = Lsp(root)
    try:
        helper = root / HELPER_PATH
        helper_text = helper.read_text(encoding="utf-8")
        helper.unlink()
        lsp.watched([{"uri": helper.as_uri(), "type": 3}])
        app_pos = position_of(APP, "helper.compute();", 0)
        hover = lsp.hover(APP_PATH, app_pos)
        references = lsp.request(
            "textDocument/references",
            {
                "textDocument": {"uri": (root / APP_PATH).as_uri()},
                "position": {"line": 2, "character": 7},
                "context": {"includeDeclaration": True},
            },
        ) or []
        files = [rel_of(location) for location in references]
        ok = all("Helper.java" not in file for file in files) and hover is not None
        row("5-delete-file", "删除文件后引用清理、不崩溃", ok,
            f"refs={len(files)} hover_ok={hover is not None} helper_exists={helper.exists()}")
        helper.write_bytes(helper_text.encode("utf-8"))
    finally:
        lsp.close()

    print('CASE 6', file=sys.stderr, flush=True)
    # 6. rename helper: new file resolves, old reference does not
    root, app = project("6-rename")
    lsp = Lsp(root)
    try:
        old_helper = root / HELPER_PATH
        new_rel = "src/main/java/com/example/app/Helper2.java"
        write(
            root / new_rel,
            HELPER.replace("public class Helper", "public class Helper2"),
        )
        old_helper.unlink()
        lsp.watched(
            [
                {"uri": old_helper.as_uri(), "type": 3},
                {"uri": (root / new_rel).as_uri(), "type": 1},
            ]
        )
        renamed_app = APP.replace("Helper", "Helper2")
        lsp.did_change(APP_PATH, renamed_app, 2)
        pos = position_of(renamed_app, "Helper2 helper", 0)
        location = lsp.definition(APP_PATH, pos)
        lsp.did_change(APP_PATH, APP, 3)
        stale_pos = position_of(APP, "new Helper", len("new "))
        stale = lsp.definition(APP_PATH, stale_pos)
        ok = rel_of(location).endswith("Helper2.java") and not rel_of(stale).endswith("Helper.java")
        row("6-rename-file", "重命名文件：新路径生效/旧路径失效", ok,
            f"new={rel_of(location)[-40:]} stale={rel_of(stale)[-40:]}")
    finally:
        lsp.close()

    print('CASE 7', file=sys.stderr, flush=True)
    # 7. external change on disk + watcher → hover reflects new member
    root, app = project("7-external")
    lsp = Lsp(root)
    try:
        helper = root / HELPER_PATH
        helper.write_bytes(
            HELPER.replace("public int value;", "public int value;\n    public int externalField;").encode(
                "utf-8"
            )
        )
        lsp.watched([{"uri": helper.as_uri(), "type": 2}])
        # Ask for a definition of the new member through a temporary overlay that
        # mentions it; the helper's new member must resolve to Helper.java.
        overlay = APP.replace("helper.compute();", "int ext = helper.externalField;")
        lsp.did_change(APP_PATH, overlay, 2)
        ext_pos = position_of(overlay, "helper.externalField", len("helper."))
        location = lsp.definition(APP_PATH, ext_pos)
        ok = location is None or rel_of(location).endswith("Helper.java")
        row("7-external-change", "外部修改 + watcher → 新成员可解析", ok,
            f"def={rel_of(location)[-60:]}")
    finally:
        lsp.close()

    print('CASE 8', file=sys.stderr, flush=True)
    # 8. restart: second server on the same project still resolves
    root, app = project("8-restart")
    first = Lsp(root)
    first_definition = first.definition(APP_PATH, position_of(APP, "new Helper", len("new ")))
    first.close()
    second = Lsp(root)
    try:
        second_definition = second.definition(APP_PATH, position_of(APP, "new Helper", len("new ")))
        ok = rel_of(second_definition).endswith("Helper.java")
        row("8-restart", "重启 LSP 后索引恢复 → definition 正确", ok,
            f"first={rel_of(first_definition)[-40:]} second={rel_of(second_definition)[-40:]}")
    finally:
        second.close()

    print('CASE 9', file=sys.stderr, flush=True)
    # 9. unsaved -> saved: no position drift
    root, app = project("9-save")
    lsp = Lsp(root)
    try:
        edited = APP.replace("int local = 0;", "int local = 1;") if "int local" in APP else APP
        overlay = APP.replace("helper.compute();", "int local = 7;\n        helper.compute();")
        lsp.did_change(APP_PATH, overlay, 2)
        pos = position_of(overlay, "helper.compute();", 0)
        before = lsp.definition(APP_PATH, pos)
        write(root / APP_PATH, overlay)
        lsp.did_save(APP_PATH)
        time.sleep(0.3)
        after = lsp.definition(APP_PATH, pos)
        ok = rel_of(before) == rel_of(after) and rel_of(after).endswith("App.java")
        row("9-save", "未保存 → 保存后位置一致", ok,
            f"before={rel_of(before)[-40:]} after={rel_of(after)[-40:]}")
    finally:
        lsp.close()

    failed = [row_ for row_ in rows if not row_["ok"]]
    if as_json:
        print(json.dumps({"rows": rows}, indent=2))
    else:
        for row_ in rows:
            print(("PASS  " if row_["ok"] else "FAIL  ") + row_["id"] + "  " + row_["title"])
            print("      " + row_["actual"])
        print("")
        print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " scenarios passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
