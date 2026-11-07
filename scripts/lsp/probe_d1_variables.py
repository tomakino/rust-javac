#!/usr/bin/env python3
"""D1 probe: definition/hover for variables and shadowing, over a real LSP session.

Starts ``target/debug/ravac-lsp.exe``, opens one fixture through
``textDocument/didOpen`` (so the unsaved-file analysis path is exercised) and
queries ``textDocument/hover`` + ``textDocument/definition`` for every D1
scenario.  Each row records the expectation and the actual answer.

Usage:
    py scripts/lsp/probe_d1_variables.py [--json]
"""

import json
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import unquote, urlparse
from urllib.request import url2pathname

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
FIXTURE_DIR = ROOT / "target" / "tmp" / "d1-probe"
FIXTURE = FIXTURE_DIR / "D1Probe.java"
URI = FIXTURE.as_uri()

TEXT = """import java.util.List;
import static java.util.Collections.emptyList;

class Base {
    int x;
    int baseOnly;
}

class D1Probe extends Base implements Runnable {
    int x;
    int e;
    int s;
    List<String> list;
    List<String> items;

    public void run() {
    }

    void localShadow() {
        int x = 1;
        int y = x;
    }

    void paramShadow(String x) {
        use(x);
    }

    void catchShadow() {
        try {
            use(null);
        } catch (Exception e) {
            use(e);
        }
    }

    void forEachShadow() {
        for (String s : list) {
            use(s);
        }
    }

    void lambdaShadow() {
        items.forEach(s -> use(s));
    }

    void methodRef() {
        items.forEach(this::use);
    }

    void thisVsBare(int x) {
        int z = x;
        this.x = 1;
    }

    void superField() {
        int v = super.x;
    }

    void use(Object o) {
    }
}

enum E {
    A,
    B
}

class EnumUse {
    E e = E.A;
}

record P(int x) {
    int twice() {
        return x * 2;
    }
}

class Box<T> {
    T value;

    T get() {
        return value;
    }
}
"""


def position(text, marker, offset):
    """LSP position of `marker` + `offset` characters (markers must be unique)."""
    start = text.index(marker) + offset
    prefix = text[:start]
    line = prefix.count("\n")
    column = start - (prefix.rfind("\n") + 1)
    return {"line": line, "character": column}


def fixture_position(marker, offset):
    return position(TEXT, marker, offset)


def uri_to_path(uri):
    return Path(unquote(url2pathname(urlparse(uri).path)))


# D1 matrix.  `ref` is the cursor; `def` is either a fixture marker+offset or a
# file suffix for definitions outside this file.
SCENARIOS = [
    {
        "id": "1-local-shadows-field",
        "title": "局部变量遮蔽字段",
        "ref": ("int y = x;", 8),
        "def": ("int x = 1;", 4),
        "hover": ["int x"],
    },
    {
        "id": "2-parameter-shadows-field",
        "title": "参数遮蔽字段",
        "ref": ("use(x);", 4),
        "def": ("paramShadow(String x) {", len("paramShadow(String ")),
        "hover": ["String x"],
    },
    {
        "id": "3-catch-parameter",
        "title": "catch 参数",
        "ref": ("use(e);", 4),
        "def": ("catch (Exception e) {", len("catch (Exception ")),
        "hover": ["Exception e"],
    },
    {
        "id": "4-enhanced-for",
        "title": "增强 for 变量",
        "ref": ("use(s);", 4),
        "def": ("for (String s : list) {", len("for (String ")),
        "hover": ["String s"],
    },
    {
        "id": "5-lambda-parameter",
        "title": "lambda 参数（遮蔽字段）",
        "ref": ("use(s))", 4),
        "def": ("forEach(s ->", len("forEach(")),
        "hover": ["String s"],
    },
    {
        "id": "6-method-reference",
        "title": "方法引用 this::use",
        "ref": ("this::use", 6),
        "def": ("void use(Object o) {", 5),
        "hover": ["use"],
    },
    {
        "id": "7a-this-field",
        "title": "this.x 字段",
        "ref": ("this.x = 1;", 5),
        "def": ("int x;\n    int e;", 4),
        "hover": ["int x"],
    },
    {
        "id": "7b-bare-parameter",
        "title": "裸 x（参数）",
        "ref": ("int z = x;", 8),
        "def": ("thisVsBare(int x) {", len("thisVsBare(int ")),
        "hover": ["int x"],
    },
    {
        "id": "8-super-field",
        "title": "super.x 父类字段",
        "ref": ("int v = super.x;", len("int v = super.")),
        "def": ("int x;\n    int baseOnly;", 4),
        "hover": ["int x"],
    },
    {
        "id": "9-enum-constant",
        "title": "枚举常量 E.A",
        "ref": ("= E.A;", 4),
        "def": ("enum E {", 13),
        "hover": ["A"],
    },
    {
        "id": "10-record-component",
        "title": "record 组件 x",
        "ref": ("return x * 2;", 7),
        "def": ("record P(int x)", len("record P(int ")),
        "hover": ["int x"],
    },
    {
        "id": "11-generic-type-parameter",
        "title": "泛型类型参数 T",
        "ref": ("T value;", 0),
        "def": ("class Box<T>", len("class Box<")),
        "hover": ["T"],
    },
    {
        "id": "12-static-import",
        "title": "静态导入 emptyList",
        "ref": ("Collections.emptyList;", len("Collections.")),
        "def": {"file_suffix": "Collections.java"},
        "hover": ["emptyList"],
    },
]


def main():
    as_json = "--json" in sys.argv
    FIXTURE_DIR.mkdir(parents=True, exist_ok=True)
    FIXTURE.write_text(TEXT, encoding="utf-8")
    if not EXE.is_file():
        print("missing " + str(EXE), file=sys.stderr)
        return 2

    proc = subprocess.Popen(
        [str(EXE)],
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

    rows = []
    try:
        send(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {"rootUri": FIXTURE_DIR.as_uri()},
            }
        )
        read()
        deadline = time.time() + 20
        while time.time() < deadline:
            send({"jsonrpc": "2.0", "id": 100, "method": "ravac/indexStats", "params": {}})
            stats = read().get("result") or {}
            if not stats.get("building"):
                break
            time.sleep(0.05)

        send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": URI,
                        "languageId": "java",
                        "version": 1,
                        "text": TEXT,
                    }
                },
            }
        )
        diagnostics = (read() or {}).get("params", {}).get("diagnostics", [])

        request_id = 10
        for scenario in SCENARIOS:
            ref = fixture_position(*scenario["ref"])
            request_id += 2
            send(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "method": "textDocument/hover",
                    "params": {"textDocument": {"uri": URI}, "position": ref},
                }
            )
            hover_result = (read() or {}).get("result")
            hover_text = ((hover_result or {}).get("contents") or {}).get("value", "")
            hover_ok = bool(hover_text) and all(
                needle in hover_text for needle in scenario["hover"]
            )

            send(
                {
                    "jsonrpc": "2.0",
                    "id": request_id + 1,
                    "method": "textDocument/definition",
                    "params": {"textDocument": {"uri": URI}, "position": ref},
                }
            )
            definition = (read() or {}).get("result")
            definition_ok = False
            actual_definition = "null"
            if definition:
                target = uri_to_path(definition["uri"])
                start = definition.get("range", {}).get("start", {})
                actual_definition = str(target) + ":" + str(start.get("line")) + ":" + str(
                    start.get("character")
                )
                if isinstance(scenario["def"], dict):
                    definition_ok = str(target).lower().endswith(
                        scenario["def"]["file_suffix"].lower()
                    )
                else:
                    expected = fixture_position(*scenario["def"])
                    definition_ok = (
                        target.name == FIXTURE.name
                        and start.get("line") == expected["line"]
                        and start.get("character") == expected["character"]
                    )
                    actual_definition += " (expected line=" + str(
                        expected["line"]
                    ) + ", character=" + str(expected["character"]) + ")"
            rows.append(
                {
                    "id": scenario["id"],
                    "title": scenario["title"],
                    "hover_ok": hover_ok,
                    "hover": hover_text.replace("\n", " | ")[:120],
                    "definition_ok": definition_ok,
                    "definition": actual_definition,
                }
            )
    finally:
        try:
            send({"jsonrpc": "2.0", "id": 999, "method": "shutdown", "params": {}})
            read()
            send({"jsonrpc": "2.0", "method": "exit", "params": {}})
            proc.wait(timeout=10)
        except Exception:
            proc.kill()

    failed = [row for row in rows if not (row["hover_ok"] and row["definition_ok"])]
    if as_json:
        print(json.dumps({"diagnostics": diagnostics, "rows": rows}, indent=2))
    else:
        print("fixture diagnostics: " + str(len(diagnostics)))
        for row in rows:
            verdict = "PASS" if row["hover_ok"] and row["definition_ok"] else "FAIL"
            print(verdict + "  " + row["id"] + "  " + row["title"])
            print("      hover: " + row["hover"])
            print("      def:   " + row["definition"])
        print("")
        print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " scenarios passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
