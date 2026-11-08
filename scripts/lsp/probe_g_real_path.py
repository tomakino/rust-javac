#!/usr/bin/env python3
"""G: real Maven path through the full stack.

One fixture project (pom + three cross-referencing classes):
  * open -> definition x3 -> completion -> rename -> didSave -> diagnostics
  * find usages of a field across the three files
  * definition from an import line (project and JDK)
  * open an extracted JDK source (StackTrace case) and query it
  * didChange syntax error -> diagnostics, fix -> cleared
  * didSave adds a method -> workspace/symbol finds it

Usage:
    py scripts/lsp/probe_g_real_path.py
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
BASE = ROOT / "target" / "tmp" / "g-real-path" / ("run-" + str(os.getpid()))
JDK_SRC = ROOT / "target" / "debug" / "ravac-lsp-out" / "jdk-src"

POM = (
    "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId>"
    "<artifactId>real</artifactId><version>1.0</version>"
    "<properties><maven.compiler.release>17</maven.compiler.release></properties></project>\n"
)
APP = """package com.example.app;

import com.example.base.Helper;
import com.example.other.User;

import java.util.List;

public class App {
    Helper helper = new Helper();
    User user = new User(helper);
    List<String> names;

    void f() {
        helper.compute();
        int v = helper.value;
        user.see(helper);
    }
}
"""
HELPER = """package com.example.base;

public class Helper {
    public int value;

    public void compute() {
    }
}
"""
USER = """package com.example.other;

import com.example.base.Helper;

public class User {
    Helper helper;

    public User(Helper helper) {
        this.helper = helper;
    }

    public void see(Helper helper) {
        int v = helper.value;
    }
}
"""
APP_PATH = "src/main/java/com/example/app/App.java"
HELPER_PATH = "src/main/java/com/example/base/Helper.java"
USER_PATH = "src/main/java/com/example/other/User.java"


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def position_of(text: str, marker: str, occurrence: int = 0, offset: int = 0):
    start = -1
    for _ in range(occurrence + 1):
        start = text.index(marker, start + 1)
    start += offset
    prefix = text[:start]
    return prefix.count("\n"), start - (prefix.rfind("\n") + 1)


def rel_of(location) -> str:
    if not location:
        return ""
    return unquote(url2pathname(urlparse(location.get("uri", "")).path)).replace("\\", "/")


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
        self.wait_ready()

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
            self.stderr_lines.append(line.decode("utf-8", "replace").strip())

    def send(self, message):
        body = json.dumps(message).encode()
        self.proc.stdin.write(b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)
        self.proc.stdin.flush()

    def read_until(self, request_id, timeout=30.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                message = self.messages.get(timeout=max(0.0, deadline - time.time()))
            except queue.Empty:
                return None
            if message.get("id") == request_id:
                return message
        return None

    def request(self, method, params, request_id=None, timeout=30.0):
        if request_id is None:
            self.next_id += 1
            request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        return (self.read_until(request_id, timeout=timeout) or {}).get("result")

    def uri(self, relative: str) -> str:
        return (self.root / relative).as_uri()

    def wait_ready(self):
        deadline = time.time() + 15
        while time.time() < deadline:
            stats = self.request("ravac/artifactStats", {}) or {}
            if stats.get("status") != "building":
                break
            time.sleep(0.02)
        # Trigger + settle the index (poll_index runs before every request).
        deadline = time.time() + 15
        while time.time() < deadline:
            self.request(
                "textDocument/definition",
                {"textDocument": {"uri": self.uri(APP_PATH)}, "position": {"line": 8, "character": 7}},
            )
            stats = self.request("ravac/indexStats", {}) or {}
            if not stats.get("building") and int(stats.get("generation") or 0) > 0:
                return
            time.sleep(0.02)

    def wait_index(self, timeout: float = 8.0):
        """Advance the index through a real query, then wait until settled."""
        deadline = time.time() + timeout
        stats = {}
        while time.time() < deadline:
            self.request("textDocument/documentSymbol", {"textDocument": {"uri": self.uri(APP_PATH)}})
            stats = self.request("ravac/indexStats", {}) or {}
            if not stats.get("building"):
                return stats
            time.sleep(0.05)
        return stats

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

    def did_open_abs(self, path: Path, text: str):
        self.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": path.as_uri(),
                        "languageId": "java",
                        "version": 1,
                        "text": text,
                    }
                },
            }
        )

    def did_close(self, relative: str):
        self.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didClose",
                "params": {"textDocument": {"uri": self.uri(relative)}},
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

    def diagnostics(self, relative: str, timeout: float = 5.0):
        uri = self.uri(relative)
        deadline = time.time() + timeout
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

    def wait_diagnostics(self, relative: str, timeout: float = 8.0, expect_empty: bool | None = None):
        deadline = time.time() + timeout
        last = None
        while time.time() < deadline:
            last = self.diagnostics(relative, timeout=max(0.1, deadline - time.time()))
            if last is None:
                return None
            if expect_empty is None:
                return last
            if expect_empty and not last:
                return last
            if not expect_empty and last:
                return last
        return last

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

    def references(self, relative, position):
        return self.request(
            "textDocument/references",
            {
                "textDocument": {"uri": self.uri(relative)},
                "position": {"line": position[0], "character": position[1]},
                "context": {"includeDeclaration": True},
            },
        )

    def rename(self, relative, position, new_name):
        return self.request(
            "textDocument/rename",
            {
                "textDocument": {"uri": self.uri(relative)},
                "position": {"line": position[0], "character": position[1]},
                "newName": new_name,
            },
        )

    def completion(self, relative, position):
        return self.request(
            "textDocument/completion",
            {
                "textDocument": {"uri": self.uri(relative)},
                "position": {"line": position[0], "character": position[1]},
            },
        )

    def workspace_symbols(self, query):
        return self.request("workspace/symbol", {"query": query})

    def close(self):
        try:
            self.proc.kill()
            self.proc.wait(timeout=5)
        except Exception:
            pass


def main():
    root = BASE / "project"
    shutil.rmtree(BASE, ignore_errors=True)
    write(root / "pom.xml", POM)
    write(root / APP_PATH, APP)
    write(root / HELPER_PATH, HELPER)
    write(root / USER_PATH, USER)

    lsp = Lsp(root)
    rows = []
    timings = {}

    def row(case_id, title, ok, actual):
        rows.append({"id": case_id, "title": title, "ok": ok, "actual": actual})

    try:
        # 1. full chain: open, definition x3, completion, rename, didSave, diagnostics
        started = time.time()
        lsp.did_open(APP_PATH, APP)
        first = lsp.wait_diagnostics(APP_PATH, timeout=8)
        timings["open"] = time.time() - started

        def timed_definition(relative, position):
            step = time.time()
            value = lsp.definition(relative, position)
            timings.setdefault("definition", []).append(round((time.time() - step) * 1000))
            return value

        d1 = timed_definition(APP_PATH, position_of(APP, "new Helper", 0, len("new ")))
        d2 = timed_definition(APP_PATH, position_of(APP, "helper.value", 0, len("helper.")))
        d3 = timed_definition(APP_PATH, position_of(APP, "new User", 0, len("new ")))
        chain_ok = (
            rel_of(d1).endswith("Helper.java")
            and rel_of(d2).endswith("Helper.java")
            and rel_of(d3).endswith("User.java")
        )
        # completion on a member access: `helper.` (temporarily edited buffer).
        edit = APP.replace("helper.compute();", "helper.")
        lsp.did_change(APP_PATH, edit, 2)
        lsp.wait_diagnostics(APP_PATH, timeout=5)
        completion = lsp.completion(APP_PATH, position_of(edit, "helper.", 0, len("helper.")))
        items = completion if isinstance(completion, list) else (completion or {}).get("items") or []
        labels = [item.get("label", "") for item in items]
        completion_ok = any("compute" in label or "value" in label for label in labels)
        # Drop the completion overlay: rename/references must see the saved
        # bytes (an open buffer is analysis-only).
        lsp.did_close(APP_PATH)
        # rename Helper -> Helper2 from the class declaration (cross-file edit).
        rename = lsp.rename(HELPER_PATH, position_of(HELPER, "class Helper", 0, len("class ")), "Helper2")
        changes = (rename or {}).get("changes") or {}
        rename_files = [unquote(url2pathname(urlparse(uri).path)).replace("\\", "/") for uri in changes]
        rename_ok = len(rename_files) >= 3 and all(
            any(uri.endswith(name) for uri in rename_files)
            for name in ("App.java", "Helper.java", "User.java")
        )
        print("stderr:", [line for line in lsp.stderr_lines if line.startswith(("[ref]", "[rename]", "[index", "[def]", "[overlay]"))][-10:], flush=True)
        total_chain = time.time() - started
        row(
            "1-full-chain",
            "打开→definition×3→completion→rename→didSave→诊断全链路",
            chain_ok and completion_ok and rename_ok,
            f"defs={[rel_of(d1)[-14:], rel_of(d2)[-14:], rel_of(d3)[-14:]]} "
            f"completion={labels[:4]} rename_files={len(rename_files)} "
            f"open={timings['open'] * 1000:.0f}ms total={total_chain * 1000:.0f}ms",
        )

        # 2. find usages of `value` from Helper: App + User + declaration.
        references = lsp.references(HELPER_PATH, position_of(HELPER, "int value", 0, len("int "))) or []
        files = sorted({rel_of(location).split("/src/")[-1] for location in references})
        five_ok = (
            len(references) >= 3
            and any("App.java" in file for file in files)
            and any("User.java" in file for file in files)
            and any("Helper.java" in file for file in files)
        )
        row(
            "2-find-usages",
            "字段 references 跨 3 文件（App/User/Helper）",
            five_ok,
            f"count={len(references)} files={files}",
        )

        # 3. definition from the import line: project import and JDK import.
        project_import = lsp.definition(APP_PATH, position_of(APP, "import com.example.base.Helper", 0, len("import com.example.base.")))
        jdk_import = lsp.definition(APP_PATH, position_of(APP, "import java.util.List", 0, len("import java.util.")))
        jdk_line = lsp.definition(APP_PATH, position_of(APP, "List<String>", 0))
        import_ok = (
            rel_of(project_import).endswith("Helper.java")
            and rel_of(jdk_import).endswith("List.java")
            and rel_of(jdk_line).endswith("List.java")
        )
        row(
            "3-import-definition",
            "import 行 definition：项目类 + JDK 类",
            import_ok,
            f"project={rel_of(project_import)[-30:]} jdk={rel_of(jdk_import)[-30:]} type={rel_of(jdk_line)[-30:]}",
        )

        # 4. extracted JDK source (StackTrace case): open + hover/definition.
        npe = JDK_SRC / "java.base/java/lang/NullPointerException.java"
        stacktrace_ok = False
        stacktrace_detail = f"missing {npe}"
        if npe.exists():
            text = npe.read_text(encoding="utf-8", errors="replace")
            lsp.did_open_abs(npe, text)
            lsp.wait_diagnostics_uri = getattr(lsp, "wait_diagnostics_uri", None)
            line, char = position_of(text, "class NullPointerException", 0, len("class "))
            hover = lsp.request(
                "textDocument/hover",
                {"textDocument": {"uri": npe.as_uri()}, "position": {"line": line, "character": char}},
                timeout=20,
            )
            super_pos = position_of(text, "extends RuntimeException", 0, len("extends "))
            definition = lsp.request(
                "textDocument/definition",
                {"textDocument": {"uri": npe.as_uri()}, "position": {"line": super_pos[0], "character": super_pos[1]}},
                timeout=20,
            )
            stacktrace_ok = hover is not None and rel_of(definition).endswith("RuntimeException.java")
            stacktrace_detail = f"hover={'ok' if hover else 'null'} def={rel_of(definition)[-30:]}"
        row("4-stacktrace-jdk", "打开 jdk-src 的 NullPointerException → hover/definition", stacktrace_ok, stacktrace_detail)

        # 5. edit diagnostics in a fresh session: the shared session already
        # queued several analysis rebuilds (single-threaded server), and G1
        # closed App so didChange would be ignored there.
        fresh = Lsp(root)
        try:
            fresh.did_open(APP_PATH, APP)
            broken = APP.replace("int v = helper.value;", "int v = helper.value")
            fresh.did_change(APP_PATH, broken, 3)
            bad = fresh.wait_diagnostics(APP_PATH, timeout=8, expect_empty=False)
            fresh.did_change(APP_PATH, APP, 4)
            clean = fresh.wait_diagnostics(APP_PATH, timeout=8, expect_empty=True)
        finally:
            fresh.close()
        diagnostics_ok = bool(bad) and clean == []
        row(
            "5-edit-diagnostics",
            "didChange 语法错误 → 诊断更新；修复 → 清空",
            diagnostics_ok,
            f"errors={len(bad or [])} cleaned={clean == []} first={((bad or [{}])[0].get('message') or '')[:40]}",
        )

        # 6. save adds a method -> workspace/symbol finds it.
        with_method = APP.replace(
            "    void f() {",
            "    void addedMethod() {\n    }\n\n    void f() {",
        )
        write(root / APP_PATH, with_method)
        lsp.did_save(APP_PATH)
        deadline = time.time() + 8
        found = []
        while time.time() < deadline:
            symbols = lsp.workspace_symbols("addedMethod") or []
            found = [symbol.get("name") for symbol in symbols]
            if any(name == "addedMethod" for name in found):
                break
            # A query advances the index; give the rebuild a moment.
            lsp.request("textDocument/documentSymbol", {"textDocument": {"uri": lsp.uri(APP_PATH)}})
            time.sleep(0.1)
        print("after save index:", lsp.request("ravac/indexStats", {}), flush=True)
        save_ok = any(name == "addedMethod" for name in found)
        row(
            "6-save-index",
            "didSave 新增方法 → workspace/symbol 可查",
            save_ok,
            f"symbols={found[:5]}",
        )
    finally:
        lsp.close()

    failed = [entry for entry in rows if not entry["ok"]]
    for entry in rows:
        print(("PASS  " if entry["ok"] else "FAIL  ") + entry["id"] + "  " + entry["title"])
        print("      " + entry["actual"])
    print("")
    print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " cases passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
