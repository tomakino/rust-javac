#!/usr/bin/env python3
"""D5 probe: foldingRange / selectionRange / documentLink / codeAction /
diagnostics / typeDefinition / implementation through a real LSP.

Every scenario opens its own file (or uses the main file opened with the same
text as its on-disk content) instead of repeatedly editing the same buffer:
this keeps the index consistent with the disk text so cross-file queries use
the index, and avoids poking the analyzer with a chain of unsaved edits.

Usage:
    py scripts/lsp/probe_d5_navigation.py [--json]
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import unquote, urlparse
from urllib.request import url2pathname

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
PROJECT = ROOT / "target" / "tmp" / ("d5-probe-project-" + str(os.getpid()))

MODEL = """package com.example.base;

public class Model {
    public int value;
}
"""
MISSING = """package com.example.base;

public class Missing {
}
"""
I = """package com.example.app;

public interface I {
    void f();
}
"""
ABSTRACT_BASE = """package com.example.app;

public abstract class AbstractBase {
    public abstract void g();
}
"""
IMPL = """package com.example.app;

public class Impl extends AbstractBase implements I {
    public void f() {
    }

    public void g() {
    }
}
"""

APP = """package com.example.app;

import com.example.base.Model;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Javadoc block
 * second line
 */
public class App extends AbstractBase implements I {
    int field;

    public void f() {
        int value = 1;
        int other = value + 1;
    }

    public void g() {
    }

    void folds() {
        if (true) {
            int a = 1;
        }
        for (int i = 0; i < 1; i++) {
            int b = 2;
        }
        while (false) {
            int c = 3;
        }
        try {
            int d = 4;
        } catch (Exception e) {
            int e2 = 5;
        }
        if (true) { int single = 6; }
    }

    void selections() {
        int value = 1;
    }

    void types() {
        List<String> list = new ArrayList<>();
        String text = "";
        var inferred = new ArrayList<String>();
    }

    void calls() {
        String made = make();
        I i = new Impl();
        i.f();
        AbstractBase base = new Impl();
        base.g();
    }

    String make() {
        return "";
    }
}
"""

GENERATE = """package com.example.app;

public class GenerateTarget {
    int value;
}
"""

# Additional files: on disk they are valid (so the index compiles cleanly); the
# probe opens some of them with a different overlay text for the scenario.
ADD_IMPORT_DISK = "package com.example.app;\n\npublic class AddImport {\n}\n"
ADD_IMPORT_OVERLAY = (
    "package com.example.app;\n\npublic class AddImport {\n"
    "    ArrayList<String> list = new ArrayList<>();\n}\n"
)
DIAG_SYNTAX = (
    "package com.example.app;\n\npublic class DiagSyntax {\n"
    "    // 中文注释：下一行是语法错误\n    void f() {\n        int x = ;\n    }\n}\n"
)
DIAG_TYPE = (
    "package com.example.app;\n\npublic class DiagType {\n"
    "    // 中文注释\n    void f() {\n        String s = 1;\n    }\n}\n"
)
DIAG_MISSING = (
    "package com.example.app;\n\npublic class DiagMissing {\n"
    "    void f() {\n        Missing m = null;\n    }\n}\n"
)
OVERLAY_FOLD = (
    "package com.example.app;\n\npublic class OverlayFold {\n"
    "    void overlay() {\n        if (true) {\n            int extra = 1;\n        }\n    }\n}\n"
)
OVERLAY_TYPE = (
    "package com.example.app;\n\nimport java.util.List;\n\npublic class OverlayType {\n"
    "    void overlay() {\n        List<String> overlayList = null;\n    }\n}\n"
)
OVERLAY_GEN = "package com.example.app;\n\npublic class OverlayGen {\n    int value;\n}\n"

APP_PATH = "src/main/java/com/example/app/App.java"
GENERATE_PATH = "src/main/java/com/example/app/GenerateTarget.java"
ADD_IMPORT_PATH = "src/main/java/com/example/app/AddImport.java"
DIAG_SYNTAX_PATH = "src/main/java/com/example/app/DiagSyntax.java"
DIAG_TYPE_PATH = "src/main/java/com/example/app/DiagType.java"
DIAG_MISSING_PATH = "src/main/java/com/example/app/DiagMissing.java"
OVERLAY_FOLD_PATH = "src/main/java/com/example/app/OverlayFold.java"
OVERLAY_TYPE_PATH = "src/main/java/com/example/app/OverlayType.java"
OVERLAY_GEN_PATH = "src/main/java/com/example/app/OverlayGen.java"

FILES = {
    "pom.xml": (
        "<project><modelVersion>4.0.0</modelVersion>"
        "<groupId>com.example</groupId><artifactId>d5-probe</artifactId>"
        "<version>1.0</version></project>\n"
    ),
    "src/main/java/com/example/base/Model.java": MODEL,
    "src/main/java/com/example/base/Missing.java": MISSING,
    "src/main/java/com/example/app/I.java": I,
    "src/main/java/com/example/app/AbstractBase.java": ABSTRACT_BASE,
    "src/main/java/com/example/app/Impl.java": IMPL,
    "src/main/java/com/example/app/App.java": APP,
    GENERATE_PATH: GENERATE,
    ADD_IMPORT_PATH: ADD_IMPORT_DISK,
    DIAG_SYNTAX_PATH: "package com.example.app;\n\npublic class DiagSyntax {\n}\n",
    DIAG_TYPE_PATH: "package com.example.app;\n\npublic class DiagType {\n}\n",
    DIAG_MISSING_PATH: "package com.example.app;\n\npublic class DiagMissing {\n}\n",
    OVERLAY_FOLD_PATH: "package com.example.app;\n\npublic class OverlayFold {\n}\n",
    OVERLAY_TYPE_PATH: "package com.example.app;\n\npublic class OverlayType {\n}\n",
    OVERLAY_GEN_PATH: "package com.example.app;\n\npublic class OverlayGen {\n}\n",
}


def line_col(text, marker, offset=0):
    start = text.index(marker) + offset
    prefix = text[:start]
    return prefix.count("\n"), start - (prefix.rfind("\n") + 1)


def rel_of(uri):
    path = str(Path(unquote(url2pathname(urlparse(uri).path)))).replace("\\", "/")
    if "/src/main/java/" in path:
        return path.split("/src/main/java/", 1)[1]
    return path.rsplit("/", 1)[-1]


def main():
    as_json = "--json" in sys.argv
    for relative, text in FILES.items():
        path = PROJECT / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text.encode("utf-8"))
    if not EXE.is_file():
        print("missing " + str(EXE), file=sys.stderr)
        return 2

    def uri(relative):
        return (PROJECT / relative).as_uri()

    proc = subprocess.Popen(
        [str(EXE)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE
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

    def read_until(request_id):
        while True:
            message = read()
            if message is None:
                return None
            if message.get("id") == request_id:
                return message

    def read_diagnostics(relative):
        short = relative.split("src/main/java/", 1)[-1]
        for _ in range(500):
            message = read()
            if message is None:
                return []
            if message.get("method") == "textDocument/publishDiagnostics":
                params = message.get("params") or {}
                if rel_of(params.get("uri", "")) == short:
                    return params.get("diagnostics") or []
        return []

    request_id = 100

    def open_file(relative, text):
        nonlocal request_id
        request_id += 1
        send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri(relative),
                        "languageId": "java",
                        "version": 1,
                        "text": text,
                    }
                },
            }
        )
        return read_diagnostics(relative)

    def request(method, relative, position, extra=None):
        nonlocal request_id
        request_id += 1
        params = {
            "textDocument": {"uri": uri(relative)},
            "position": {"line": position[0], "character": position[1]},
        }
        if extra:
            params.update(extra)
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params,
            }
        )
        return (read_until(request_id) or {}).get("result")

    rows = []
    try:
        send(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {"rootUri": PROJECT.as_uri()},
            }
        )
        read_until(1)
        deadline = time.time() + 15
        while time.time() < deadline:
            request_id += 1
            send({"jsonrpc": "2.0", "id": request_id, "method": "ravac/indexStats", "params": {}})
            stats = (read_until(request_id) or {}).get("result") or {}
            if not stats.get("building") and stats.get("generation", 0) > 0:
                break
            time.sleep(0.05)
        open_file(APP_PATH, APP)

        class_end = len(APP.splitlines()) - 1

        # 1: foldingRange
        request_id += 1
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "textDocument/foldingRange",
                "params": {"textDocument": {"uri": uri(APP_PATH)}},
            }
        )
        ranges = (read_until(request_id) or {}).get("result") or []
        imported = any(range_.get("kind") == "imports" for range_ in ranges)
        commented = any(range_.get("kind") == "comment" for range_ in ranges)
        class_body = any(
            range_.get("startLine") == 11 and range_.get("endLine") == class_end
            for range_ in ranges
        )
        method_body = any(
            range_.get("startLine") == 14 and range_.get("endLine") == 17
            for range_ in ranges
        )
        block_starts = {23, 26, 29, 32}
        covered_blocks = {
            range_.get("startLine")
            for range_ in ranges
            if range_.get("startLine") in block_starts
        }
        single_line = any(
            range_.get("startLine") == 37 and range_.get("endLine") == 37
            for range_ in ranges
        )
        ok = (
            imported
            and commented
            and class_body
            and method_body
            and covered_blocks == block_starts
            and not single_line
        )
        actual = (
            "n=" + str(len(ranges))
            + " imports=" + str(imported)
            + " comment=" + str(commented)
            + " class=" + str(class_body)
            + " method=" + str(method_body)
            + " blocks=" + str(sorted(covered_blocks))
            + " single=" + str(single_line)
        )
        rows.append({"id": "1-folding", "title": "foldingRange 块/Javadoc/import/单行块", "ok": ok, "actual": actual})

        # 2: selectionRange
        cursor = line_col(APP, "void selections() {\n        int value = 1;", len("void selections() {\n        "))
        request_id += 1
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "textDocument/selectionRange",
                "params": {
                    "textDocument": {"uri": uri(APP_PATH)},
                    "positions": [{"line": cursor[0], "character": cursor[1]}],
                },
            }
        )
        chain = (read_until(request_id) or {}).get("result") or []
        ok = False
        actual = "empty"
        if chain:
            node = chain[0]
            levels = []
            while node:
                range_ = node.get("range") or {}
                start = range_.get("start") or {}
                end = range_.get("end") or {}
                levels.append((start.get("line"), start.get("character"), end.get("line"), end.get("character")))
                node = node.get("parent")
            contains = all(
                (start_line < cursor[0] or (start_line == cursor[0] and start_col <= cursor[1]))
                and (end_line > cursor[0] or (end_line == cursor[0] and end_col >= cursor[1]))
                for start_line, start_col, end_line, end_col in levels
            )
            nested = all(
                (levels[index + 1][0], levels[index + 1][1])
                <= (levels[index][0], levels[index][1])
                and (levels[index + 1][2], levels[index + 1][3])
                >= (levels[index][2], levels[index][3])
                for index in range(len(levels) - 1)
            )
            ok = len(levels) >= 3 and contains and nested and levels[-1][2] >= class_end
            actual = "levels=" + str(levels)
        rows.append({"id": "2-selection", "title": "selectionRange 链（词→语句→块→方法→类）", "ok": ok, "actual": actual})

        # 3: documentLink
        request_id += 1
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "textDocument/documentLink",
                "params": {"textDocument": {"uri": uri(APP_PATH)}},
            }
        )
        links = (read_until(request_id) or {}).get("result") or []
        targets = [link.get("target", "") for link in links]
        ok = any(target.endswith("Model.java") for target in targets) and any(
            "List.java" in target for target in targets
        )
        rows.append({"id": "3a-document-link", "title": "documentLink import 跳转（项目/JDK）", "ok": ok, "actual": "n=" + str(len(links)) + " targets=" + str(targets[:6])})

        request_id += 1
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "textDocument/documentLink",
                "params": {"textDocument": {"uri": uri(GENERATE_PATH)}},
            }
        )
        links = (read_until(request_id) or {}).get("result") or []
        rows.append({"id": "3b-document-link-empty", "title": "documentLink 无 import 文件为空", "ok": links == [], "actual": str(links)})

        # 4: codeAction
        open_file(ADD_IMPORT_PATH, ADD_IMPORT_OVERLAY)
        missing_pos = line_col(ADD_IMPORT_OVERLAY, "ArrayList<String>", 0)
        actions = request("textDocument/codeAction", ADD_IMPORT_PATH, missing_pos, {"range": {"start": {"line": missing_pos[0], "character": missing_pos[1]}, "end": {"line": missing_pos[0], "character": missing_pos[1]}}}) or []
        titles = [action.get("title", "") for action in actions]
        ok = any(title.startswith("Import '") for title in titles)
        rows.append({"id": "4a-add-import", "title": "codeAction add import（未解析符号）", "ok": ok, "actual": str(titles)})

        import_pos = line_col(APP, "import java.util.Collections;", 0)
        actions = request("textDocument/codeAction", APP_PATH, import_pos, {"range": {"start": {"line": import_pos[0], "character": import_pos[1]}, "end": {"line": import_pos[0], "character": import_pos[1]}}}) or []
        titles = [action.get("title", "") for action in actions]
        rows.append({"id": "4b-remove-unused", "title": "codeAction remove unused import", "ok": any("unused" in title.lower() for title in titles), "actual": str(titles)})
        rows.append({"id": "4c-organize", "title": "codeAction organize imports", "ok": any(title == "Organize imports" for title in titles), "actual": str(titles)})

        generate_pos = line_col(GENERATE, "public class GenerateTarget", len("public class "))
        actions = request("textDocument/codeAction", GENERATE_PATH, generate_pos, {"range": {"start": {"line": generate_pos[0], "character": generate_pos[1]}, "end": {"line": generate_pos[0], "character": generate_pos[1]}}}) or []
        titles = [action.get("title", "") for action in actions]
        wanted = ["Generate constructor", "Generate toString()", "Generate equals() and hashCode()"]
        rows.append({"id": "4d-generate", "title": "codeAction generate（构造器/toString/equals+hashCode）", "ok": all(any(title == expected for title in titles) for expected in wanted), "actual": str(titles)})

        # 5: diagnostics (separate files opened with an erroneous overlay)
        for case_id, relative, text, error_line in [
            ("5a-syntax-utf16", DIAG_SYNTAX_PATH, DIAG_SYNTAX, "int x = ;"),
            ("5b-type-error", DIAG_TYPE_PATH, DIAG_TYPE, "String s = 1;"),
            ("5c-unresolved", DIAG_MISSING_PATH, DIAG_MISSING, "Missing m = null;"),
        ]:
            diagnostics = open_file(relative, text)
            expected_line = text[: text.index(error_line)].count("\n")
            ok = any(
                (diagnostic.get("range") or {}).get("start", {}).get("line") == expected_line
                for diagnostic in diagnostics
            )
            actual = (
                "n=" + str(len(diagnostics))
                + " wantLine=" + str(expected_line)
                + " lines=" + str([(diagnostic.get("range") or {}).get("start", {}).get("line") for diagnostic in diagnostics])
                + " messages=" + str([diagnostic.get("message") for diagnostic in diagnostics[:3]])
            )
            rows.append({"id": case_id, "title": "diagnostics（中文注释后错误行不偏移）", "ok": ok, "actual": actual})

        # 6: typeDefinition
        list_pos = line_col(APP, "List<String> list", 0)
        location = request("textDocument/typeDefinition", APP_PATH, list_pos)
        rows.append({"id": "6a-type-list", "title": "typeDefinition List<String> 的 List", "ok": bool(location) and "List.java" in location.get("uri", ""), "actual": json.dumps(location)[:160]})

        make_pos = line_col(APP, "make();", 0)
        location = request("textDocument/typeDefinition", APP_PATH, make_pos)
        rows.append({"id": "6b-type-return", "title": "typeDefinition 方法返回类型 make() → String", "ok": bool(location) and "String.java" in location.get("uri", ""), "actual": json.dumps(location)[:160]})

        var_pos = line_col(APP, "var inferred", len("var "))
        location = request("textDocument/typeDefinition", APP_PATH, var_pos)
        rows.append({"id": "6c-type-var", "title": "typeDefinition var 推断类型", "ok": bool(location) and "ArrayList.java" in location.get("uri", ""), "actual": json.dumps(location)[:160]})

        # 7: implementation (index-backed). The earlier scenarios opened and
        # edited buffers; their overlay rebuilds race with the query (the
        # server keeps serving the previous snapshot while a build is in
        # flight), so ask a fresh server with a completed project index and no
        # overlays: this is the normal "open a project and navigate" state.
        interface_pos = line_col(APP, "I i = new Impl();", 0)
        abstract_pos = line_col(APP, "base.g();", len("base."))
        override_pos = line_col(APP, "i.f();", len("i."))
        fresh = subprocess.Popen(
            [str(EXE)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        def fresh_send(message):
            body = json.dumps(message).encode()
            fresh.stdin.write(b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)
            fresh.stdin.flush()
        def fresh_read():
            headers = b""
            while b"\r\n\r\n" not in headers:
                chunk = fresh.stdout.read(1)
                if not chunk:
                    return None
                headers += chunk
            length = int(headers.split(b"Content-Length: ")[1].split(b"\r\n")[0])
            return json.loads(fresh.stdout.read(length))
        def fresh_until(rid):
            while True:
                message = fresh_read()
                if message is None:
                    return None
                if message.get("id") == rid:
                    return message
        fresh_send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": PROJECT.as_uri()}})
        fresh_until(1)
        fresh_id = 100
        deadline = time.time() + 15
        while time.time() < deadline:
            fresh_id += 1
            fresh_send({"jsonrpc": "2.0", "id": fresh_id, "method": "ravac/indexStats", "params": {}})
            stats = (fresh_until(fresh_id) or {}).get("result") or {}
            if not stats.get("building") and stats.get("generation", 0) > 0:
                break
            time.sleep(0.05)
        fresh_results = []
        for position in (interface_pos, abstract_pos, override_pos):
            fresh_id += 1
            fresh_send(
                {
                    "jsonrpc": "2.0",
                    "id": fresh_id,
                    "method": "textDocument/implementation",
                    "params": {
                        "textDocument": {"uri": uri(APP_PATH)},
                        "position": {"line": position[0], "character": position[1]},
                    },
                }
            )
            fresh_results.append((fresh_until(fresh_id) or {}).get("result") or [])
        try:
            fresh_send({"jsonrpc": "2.0", "id": 9998, "method": "shutdown", "params": {}})
            fresh_until(9998)
            fresh_send({"jsonrpc": "2.0", "method": "exit", "params": {}})
            fresh.wait(timeout=10)
        except Exception:
            fresh.kill()
        for case_id, title, locations in [
            ("7a-interface", "implementation 接口 I", fresh_results[0]),
            ("7b-abstract", "implementation 抽象方法 g", fresh_results[1]),
            ("7c-override", "implementation 方法重写 f", fresh_results[2]),
        ]:
            files = [rel_of(location.get("uri", "")) for location in locations]
            rows.append(
                {
                    "id": case_id,
                    "title": title,
                    "ok": any("Impl.java" in file for file in files),
                    "actual": "n=" + str(len(locations)) + " files=" + str(files),
                }
            )

        # 8: unsaved overlay spot checks (separate files, folding / typeDefinition / generate)
        open_file(OVERLAY_FOLD_PATH, OVERLAY_FOLD)
        request_id += 1
        send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "textDocument/foldingRange",
                "params": {"textDocument": {"uri": uri(OVERLAY_FOLD_PATH)}},
            }
        )
        ranges = (read_until(request_id) or {}).get("result") or []
        method_fold = any(range_.get("startLine") == 3 and range_.get("endLine") == 7 for range_ in ranges)
        if_fold = any(range_.get("startLine") == 4 and range_.get("endLine") == 6 for range_ in ranges)
        rows.append({"id": "8a-overlay-folding", "title": "overlay folding 反映新块", "ok": method_fold and if_fold, "actual": "n=" + str(len(ranges)) + " ranges=" + str(ranges)})

        open_file(OVERLAY_TYPE_PATH, OVERLAY_TYPE)
        list_pos = line_col(OVERLAY_TYPE, "List<String> overlayList", 0)
        location = request("textDocument/typeDefinition", OVERLAY_TYPE_PATH, list_pos)
        rows.append({"id": "8b-overlay-type", "title": "overlay typeDefinition List", "ok": bool(location) and "List.java" in location.get("uri", ""), "actual": json.dumps(location)[:160]})

        open_file(OVERLAY_GEN_PATH, OVERLAY_GEN)
        generate_pos = line_col(OVERLAY_GEN, "public class OverlayGen", len("public class "))
        actions = request("textDocument/codeAction", OVERLAY_GEN_PATH, generate_pos, {"range": {"start": {"line": generate_pos[0], "character": generate_pos[1]}, "end": {"line": generate_pos[0], "character": generate_pos[1]}}}) or []
        titles = [action.get("title", "") for action in actions]
        rows.append({"id": "8c-overlay-generate", "title": "overlay generate toString", "ok": any(title == "Generate toString()" for title in titles), "actual": str(titles)})
    finally:
        try:
            send({"jsonrpc": "2.0", "id": 9999, "method": "shutdown", "params": {}})
            read_until(9999)
            send({"jsonrpc": "2.0", "method": "exit", "params": {}})
            proc.wait(timeout=10)
        except Exception:
            proc.kill()

    failed = [row for row in rows if not row["ok"]]
    if as_json:
        print(json.dumps({"rows": rows}, indent=2))
    else:
        for row in rows:
            print(("PASS  " if row["ok"] else "FAIL  ") + row["id"] + "  " + row["title"])
            print("      " + row["actual"])
        print("")
        print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " scenarios passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
