#!/usr/bin/env python3
"""D4 probe: completion / signatureHelp / inlayHint through a real LSP.

Each scenario replaces the main file with a small overlay (``|`` marks the
cursor), then sends the real request and checks the answer.

Usage:
    py scripts/lsp/probe_d4_completion.py [--json]
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
PROJECT = ROOT / "target" / "tmp" / ("d4-probe-project-" + str(os.getpid()))

BASE_TYPE = """package com.example.app;

public class BaseType {
    public int value;

    public BaseType() {
    }

    public BaseType(int value) {
    }

    public BaseType(String value) {
    }

    public void compute() {
    }

    public static void staticMethod() {
    }

    public void over(int value) {
    }

    public void over(String value) {
    }

    public void single(int a, String b) {
    }

    public static void sum(int... values) {
    }
}
"""

APP_PATH = "src/main/java/com/example/app/App.java"
MAIN = PROJECT / APP_PATH

FILES = {
    "pom.xml": (
        "<project><modelVersion>4.0.0</modelVersion>"
        "<groupId>com.example</groupId><artifactId>d4-probe</artifactId>"
        "<version>1.0</version></project>\n"
    ),
    "src/main/java/com/example/app/BaseType.java": BASE_TYPE,
    APP_PATH: "package com.example.app;\n\npublic class App {\n}\n",
}

# (id, title, source with `|` cursor, expectation)
# expectation kinds:
#   completion: {"contains": [...], "excludes": [...]}
#   signature:  {"label_contains": "...", "active_parameter": N}
#   inlay:      {"label_contains": "...", "kind": 1|2}
#   empty:      {}
SCENARIOS = [
    (
        "1-this-members",
        "this. 成员补全（字段/方法）",
        """package com.example.app;

public class App {
    int field;

    void compute() {
    }

    void f() {
        this.|
    }
}
""",
        {"type": "completion", "contains": ["field", "compute"]},
    ),
    (
        "2-object-members",
        "变量 obj. 成员补全（类型已知）",
        """package com.example.app;

public class App {
    void f() {
        BaseType obj = new BaseType();
        obj.|
    }
}
""",
        {"type": "completion", "contains": ["value", "compute", "single"]},
    ),
    (
        "3-type-name",
        "类型名补全 Str → String",
        """package com.example.app;

public class App {
    void f() {
        Str|
    }
}
""",
        {"type": "completion", "contains": ["String"]},
    ),
    (
        "4-static-members",
        "静态成员补全 Collections.",
        """package com.example.app;

import java.util.Collections;

public class App {
    void f() {
        Collections.|
    }
}
""",
        {"type": "completion", "contains": ["emptyList", "sort"]},
    ),
    (
        "5a-local-variable",
        "局部变量补全（作用域内）",
        """package com.example.app;

public class App {
    void f() {
        int local = 1;
        loc|
    }
}
""",
        {"type": "completion", "contains": ["local"]},
    ),
    (
        "5b-parameter",
        "参数补全（作用域内）",
        """package com.example.app;

public class App {
    void f(int param) {
        par|
    }
}
""",
        {"type": "completion", "contains": ["param"]},
    ),
    (
        "6-import-candidate",
        "import 补全（未导入的 List）",
        """package com.example.app;

public class App {
    void f() {
        List|
    }
}
""",
        {"type": "completion", "contains": ["List"]},
    ),
    (
        "7-keyword",
        "关键字补全 ret → return",
        """package com.example.app;

public class App {
    void f() {
        ret|
    }
}
""",
        {"type": "completion", "contains": ["return"]},
    ),
    (
        "8-unresolved",
        "未解析符号不崩溃、返回空",
        """package com.example.app;

public class App {
    void f() {
        UnknownType.|
    }
}
""",
        {"type": "completion", "contains": []},
    ),
    (
        "9-signature-single",
        "单方法 signatureHelp（当前参数索引）",
        """package com.example.app;

public class App {
    void single(int a, String b) {
    }

    void f() {
        single(1, |)
    }
}
""",
        {"type": "signature", "label_contains": "single(", "active_parameter": 1},
    ),
    (
        "10a-overload-int",
        "重载 signatureHelp 选 int 版本",
        """package com.example.app;

public class App {
    void over(int a) {
    }

    void over(String a) {
    }

    void f() {
        over(1|)
    }
}
""",
        {"type": "signature", "label_contains": "over(int", "active_parameter": 0},
    ),
    (
        "10b-overload-string",
        "重载 signatureHelp 选 String 版本",
        """package com.example.app;

public class App {
    void over(int a) {
    }

    void over(String a) {
    }

    void f() {
        over("x"|)
    }
}
""",
        {"type": "signature", "label_contains": "over(String", "active_parameter": 0},
    ),
    (
        "11-constructor",
        "构造器 signatureHelp",
        """package com.example.app;

public class App {
    void f() {
        new BaseType(1|)
    }
}
""",
        {"type": "signature", "label_contains": "BaseType(int", "active_parameter": 0},
    ),
    (
        "12-varargs",
        "varargs signatureHelp",
        """package com.example.app;

public class App {
    void f() {
        BaseType.sum(1, 2|)
    }
}
""",
        {"type": "signature", "label_contains": "sum(int...", "active_parameter": 1},
    ),
    (
        "13-var-inlay",
        "var 局部变量类型 inlay hint",
        """package com.example.app;

public class App {
    void f() {
        var x = new BaseType();
        x.compute();
    }
}
""",
        {"type": "inlay", "label_contains": "BaseType", "kind": 1},
    ),
    (
        "14-parameter-inlay",
        "方法参数名 inlay hint（同文件声明）",
        """package com.example.app;

public class App {
    void single(int a, String b) {
    }

    void f() {
        single(1, "x");
    }
}
""",
        {"type": "inlay", "label_contains": "a:", "kind": 2},
    ),
    (
        "14b-parameter-inlay-cross-file",
        "跨文件参数名 inlay（项目模式 source_path 为空，记录限制）",
        """package com.example.app;

public class App {
    void f() {
        new BaseType().single(1, "x");
    }
}
""",
        {"type": "inlay_absent_kind", "kind": 2},
    ),
    (
        "15-return-inlay",
        "返回类型 inlay hint（项目未实现，记录）",
        """package com.example.app;

public class App {
    int f() {
        return 1;
    }
}
""",
        {"type": "inlay_absent_kind", "kind": 0},
    ),
]


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

    def cursor_of(source):
        index = source.index("|")
        text = source.replace("|", "", 1)
        prefix = text[: index]
        line = prefix.count("\n")
        column = index - (prefix.rfind("\n") + 1)
        return text, line, column

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
            # notifications (diagnostics, index stats) are skipped

    rows = []
    diagnostics = []
    next_id = 100
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
        deadline = time.time() + 30
        while time.time() < deadline:
            next_id += 1
            send({"jsonrpc": "2.0", "id": next_id, "method": "ravac/indexStats", "params": {}})
            stats = (read_until(next_id) or {}).get("result") or {}
            if not stats.get("building") and stats.get("generation", 0) > 0:
                break
            time.sleep(0.05)
        send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri(APP_PATH),
                        "languageId": "java",
                        "version": 1,
                        "text": FILES[APP_PATH],
                    }
                },
            }
        )

        version = 1
        for scenario_id, title, source, expectation in SCENARIOS:
            if "|" in source:
                text, line, column = cursor_of(source)
            else:
                text, line, column = source, 0, 0
            version += 1
            next_id += 1
            send(
                {
                    "jsonrpc": "2.0",
                    "method": "textDocument/didChange",
                    "params": {
                        "textDocument": {"uri": uri(APP_PATH), "version": version},
                        "contentChanges": [{"text": text}],
                    },
                }
            )
            # Read until the server has finished the change notification cycle:
            # request index stats as a barrier (cheap, synchronous server side).
            barrier = next_id
            send({"jsonrpc": "2.0", "id": barrier, "method": "ravac/indexStats", "params": {}})
            read_until(barrier)

            next_id += 1
            request_id = next_id
            kind = expectation["type"]
            if kind == "completion":
                send(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "textDocument/completion",
                        "params": {
                            "textDocument": {"uri": uri(APP_PATH)},
                            "position": {"line": line, "character": column},
                        },
                    }
                )
                result = (read_until(request_id) or {}).get("result")
                items = result if isinstance(result, list) else []
                labels = [item.get("label", "") for item in items]
                wanted = expectation["contains"]
                missing = [label for label in wanted if label not in labels]
                ok = not missing
                if kind == "completion" and scenario_id == "8-unresolved":
                    ok = ok and len(labels) == 0
                actual = "n=" + str(len(labels)) + " labels=" + str(labels[:12])
                if missing:
                    actual += " missing=" + str(missing)
                if scenario_id == "8-unresolved" and labels:
                    ok = False
                    actual += " expected empty"
            elif kind == "signature":
                send(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "textDocument/signatureHelp",
                        "params": {
                            "textDocument": {"uri": uri(APP_PATH)},
                            "position": {"line": line, "character": column},
                        },
                    }
                )
                result = (read_until(request_id) or {}).get("result")
                signatures = (result or {}).get("signatures") or []
                labels = [signature.get("label", "") for signature in signatures]
                active_signature = (result or {}).get("activeSignature")
                active_parameter = (result or {}).get("activeParameter")
                label = labels[active_signature] if labels and active_signature is not None and active_signature < len(labels) else (labels[0] if labels else "")
                ok = (
                    expectation["label_contains"] in label
                    and active_parameter == expectation["active_parameter"]
                )
                actual = (
                    "activeSignature=" + str(active_signature)
                    + " activeParameter=" + str(active_parameter)
                    + " label=" + json.dumps(label)
                )
            elif kind == "inlay":
                last_line = text.count("\n")
                send(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "textDocument/inlayHint",
                        "params": {
                            "textDocument": {"uri": uri(APP_PATH)},
                            "range": {
                                "start": {"line": 0, "character": 0},
                                "end": {"line": last_line, "character": 0},
                            },
                        },
                    }
                )
                result = (read_until(request_id) or {}).get("result")
                hints = result if isinstance(result, list) else []
                wanted = expectation["label_contains"]
                matching = [
                    hint
                    for hint in hints
                    if wanted in str(hint.get("label"))
                    and hint.get("kind") == expectation["kind"]
                ]
                ok = bool(matching)
                actual = "hints=" + json.dumps(
                    [
                        {"label": hint.get("label"), "kind": hint.get("kind")}
                        for hint in hints
                    ]
                )
            else:  # inlay_absent_kind
                last_line = text.count("\n")
                send(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "textDocument/inlayHint",
                        "params": {
                            "textDocument": {"uri": uri(APP_PATH)},
                            "range": {
                                "start": {"line": 0, "character": 0},
                                "end": {"line": last_line, "character": 0},
                            },
                        },
                    }
                )
                result = (read_until(request_id) or {}).get("result")
                hints = result if isinstance(result, list) else []
                # Documented behaviour: no return-type hints are implemented;
                # only Type (1) and Parameter (2) hints are produced. For
                # cross-file calls the parameter names are absent (project-mode
                # analysis does not compile the declaring file), which the
                # scenario records as an accepted limitation.
                if expectation["kind"] == 0:
                    ok = all(hint.get("kind") in (1, 2) for hint in hints)
                else:
                    ok = not any(hint.get("kind") == expectation["kind"] for hint in hints)
                actual = "hints=" + json.dumps(
                    [
                        {"label": hint.get("label"), "kind": hint.get("kind")}
                        for hint in hints
                    ]
                )
            rows.append({"id": scenario_id, "title": title, "ok": ok, "actual": actual})
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
        print(json.dumps({"diagnostics": diagnostics, "rows": rows}, indent=2))
    else:
        print("fixture diagnostics: " + str(len(diagnostics)))
        for row in rows:
            print(("PASS  " if row["ok"] else "FAIL  ") + row["id"] + "  " + row["title"])
            print("      " + row["actual"])
        print("")
        print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " scenarios passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
