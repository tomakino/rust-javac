#!/usr/bin/env python3
"""D3 probe: references / rename / prepareRename semantics through a real LSP.

Initializes a multi-file Maven project, opens the main file with
``textDocument/didOpen`` and checks every D3 scenario against the actual
``textDocument/references``, ``textDocument/rename`` and
``textDocument/prepareRename`` answers.

Usage:
    py scripts/lsp/probe_d3_references.py [--json] [--no-open]
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
PROJECT = ROOT / "target" / "tmp" / ("d3-probe-project-" + str(os.getpid()))

MODEL = """package com.example.base;

public class Model {
    public int value;

    public void compute() {
    }
}
"""

TAG = """package com.example.base;

public @interface Tag {
}
"""

A_FOO = """package com.example.a;

public class Foo {
    public int a;
}
"""

B_FOO = """package com.example.b;

public class Foo {
    public int b;
}
"""

CHILD = """package com.example.app;

import com.example.base.Model;

public class Child extends Model {
    @Override
    public void compute() {
    }
}
"""

USER = """package com.example.app;

import com.example.base.Model;

class User {
    void use(Model model) {
        model.value = 1;
        int x = model.value;
        model.compute();
    }
}
"""

USER2 = """package com.example.app;

import com.example.base.Model;

class User2 {
    void use(Model model) {
        model.value++;
    }
}
"""

APP = """package com.example.app;

import com.example.base.Model;
import com.example.base.Tag;
import java.util.List;

@Tag
public class App {
    int field;
    Model model;

    void fields() {
        this.field = 1;
        int x = this.field;
        // field in a comment
        String s = "field";
        Model m = new Model();
        m.value = 2;
        int y = m.value;
    }

    void methods() {
        compute();
        this.compute();
        Runnable r = model::compute;
        Model ref = new Child();
        ref.compute();
    }

    void compute() {
    }

    void locals() {
        int x = 1;
        int y = x;
    }

    void locals2() {
        int x = 2;
        int y = x;
    }

    void params(int p) {
        int y = p;
    }

    void catchParam() {
        try {
            compute();
        } catch (Exception e) {
            String s = e.getMessage();
        }
    }

    void forVar() {
        for (String s : List.of("a")) {
            String t = s;
        }
    }

    void lambdaParam() {
        List.of("a").forEach(s -> {
            String u = s;
        });
    }

    void shadow() {
        int field = 0;
        int y = field;
    }

    void qualify(Model other) {
        other.value = 1;
    }

    void generic() {
        List<Model> models = null;
    }

    void samePackageNames() {
        com.example.a.Foo first = null;
        com.example.b.Foo second = null;
    }
}
"""

FILES = {
    "pom.xml": (
        "<project><modelVersion>4.0.0</modelVersion>"
        "<groupId>com.example</groupId><artifactId>d3-probe</artifactId>"
        "<version>1.0</version></project>\n"
    ),
    "src/main/java/com/example/base/Model.java": MODEL,
    "src/main/java/com/example/base/Tag.java": TAG,
    "src/main/java/com/example/a/Foo.java": A_FOO,
    "src/main/java/com/example/b/Foo.java": B_FOO,
    "src/main/java/com/example/app/Child.java": CHILD,
    "src/main/java/com/example/app/User.java": USER,
    "src/main/java/com/example/app/User2.java": USER2,
    "src/main/java/com/example/app/App.java": APP,
}

MAIN_REL = "src/main/java/com/example/app/App.java"
TEXTS = {
    (relative.split("src/main/java/", 1)[1] if "src/main/java/" in relative else relative): text
    for relative, text in FILES.items()
}
FULL = {
    (relative.split("src/main/java/", 1)[1] if "src/main/java/" in relative else relative): relative
    for relative in FILES
}

# Relative file names used in expectations (shorter, unambiguous).
MODEL_R = "com/example/base/Model.java"
TAG_R = "com/example/base/Tag.java"
CHILD_R = "com/example/app/Child.java"
USER_R = "com/example/app/User.java"
USER2_R = "com/example/app/User2.java"
APP_R = "com/example/app/App.java"


def position(text, marker, offset):
    start = text.index(marker) + offset
    prefix = text[:start]
    return {
        "line": prefix.count("\n"),
        "character": start - (prefix.rfind("\n") + 1),
    }


def rel_of(path):
    path = str(path).replace("\\", "/")
    marker = "/src/main/java/"
    if marker in path:
        return path.split(marker, 1)[1]
    return path.rsplit("/", 1)[-1]


def req(file_rel, marker, offset):
    return (file_rel, marker, offset)


def expect_refs(*items):
    return {"kind": "references", "expect": list(items)}


def expect_edits(*items):
    return {"kind": "rename", "expect_edits": list(items)}


SCENARIOS = [
    {
        "id": "1-field-references",
        "title": "字段 references（this.x 两处，声明）",
        "ref": req(APP_R, "this.field = 1;", 5),
        "expect": expect_refs(
            req(APP_R, "    int field;", 8),
            req(APP_R, "this.field = 1;", 5),
            req(APP_R, "int x = this.field;", 13),
        ),
    },
    {
        "id": "2-method-references",
        "title": "方法 references（调用 + A::method 方法引用）",
        "ref": req(MODEL_R, "public void compute() {", 12),
        "expect": expect_refs(
            req(MODEL_R, "public void compute() {", 12),
            req(USER_R, "model.compute();", 6),
            req(APP_R, "model::compute", 7),
            req(APP_R, "ref.compute();", 4),
            # `Child.compute` is a distinct override symbol: the probe records
            # whether the convention includes it (it currently does not).
        ),
    },
    {
        "id": "3-class-references",
        "title": "类 references（声明/import/字段/new/泛型/extend）",
        "ref": req(MODEL_R, "public class Model {", 13),
        "expect": expect_refs(
            req(MODEL_R, "public class Model {", 13),
            req(USER_R, "import com.example.base.Model;", 24),
            req(USER_R, "void use(Model model)", 9),
            req(USER2_R, "import com.example.base.Model;", 24),
            req(USER2_R, "void use(Model model)", 9),
            req(APP_R, "import com.example.base.Model;", 24),
            req(APP_R, "    Model model;", 4),
            req(APP_R, "Model m = new Model();", 0),
            req(APP_R, "new Model()", 4),
            req(APP_R, "Model ref = new Child();", 0),
            req(APP_R, "List<Model> models", 5),
            req(APP_R, "void qualify(Model other)", 13),
            req(CHILD_R, "import com.example.base.Model;", 24),
            req(CHILD_R, "extends Model", 8),
        ),
    },
    {
        "id": "4-local-references",
        "title": "局部变量 references（同作用域，不同方法不混）",
        "ref": req(APP_R, "int x = 1;", 4),
        "expect": expect_refs(
            req(APP_R, "int x = 1;", 4),
            req(APP_R, "int y = x;", 8),
        ),
    },
    {
        "id": "5a-parameter-references",
        "title": "参数 references",
        "ref": req(APP_R, "void params(int p)", 16),
        "expect": expect_refs(
            req(APP_R, "void params(int p)", 16),
            req(APP_R, "int y = p;", 8),
        ),
    },
    {
        "id": "5b-catch-parameter-references",
        "title": "catch 参数 references",
        "ref": req(APP_R, "catch (Exception e)", 17),
        "expect": expect_refs(
            req(APP_R, "catch (Exception e)", 17),
            req(APP_R, "String s = e.getMessage();", 11),
        ),
    },
    {
        "id": "5c-for-variable-references",
        "title": "增强 for 变量 references",
        "ref": req(APP_R, "for (String s : List.of(\"a\"))", 12),
        "expect": expect_refs(
            req(APP_R, "for (String s : List.of(\"a\"))", 12),
            req(APP_R, "String t = s;", 11),
        ),
    },
    {
        "id": "5d-lambda-parameter-references",
        "title": "lambda 参数 references",
        "ref": req(APP_R, "forEach(s ->", 8),
        "expect": expect_refs(
            req(APP_R, "forEach(s ->", 8),
            req(APP_R, "String u = s;", 11),
        ),
    },
    {
        "id": "6-shadowed-field-references",
        "title": "遮蔽：局部 field 的 references 不含 this.field",
        "ref": req(APP_R, "int field = 0;", 4),
        "expect": expect_refs(
            req(APP_R, "int field = 0;", 4),
            req(APP_R, "int y = field;", 8),
        ),
    },
    {
        "id": "7-cross-file-field-references",
        "title": "跨文件字段 references（User/User2/App）",
        "ref": req(MODEL_R, "public int value;", 11),
        "expect": expect_refs(
            req(MODEL_R, "public int value;", 11),
            req(USER_R, "model.value = 1;", 6),
            req(USER_R, "int x = model.value;", 14),
            req(USER2_R, "model.value++;", 6),
            req(APP_R, "m.value = 2;", 2),
            req(APP_R, "int y = m.value;", 10),
            req(APP_R, "other.value = 1;", 6),
        ),
    },
    {
        "id": "8a-same-name-class-a",
        "title": "同名类 a.Foo references（不串 b.Foo）",
        "ref": req("com/example/a/Foo.java", "public class Foo {", 13),
        "expect": expect_refs(
            req("com/example/a/Foo.java", "public class Foo {", 13),
            req(APP_R, "com.example.a.Foo first", 14),
        ),
    },
    {
        "id": "8b-same-name-class-b",
        "title": "同名类 b.Foo references（不串 a.Foo）",
        "ref": req("com/example/b/Foo.java", "public class Foo {", 13),
        "expect": expect_refs(
            req("com/example/b/Foo.java", "public class Foo {", 13),
            req(APP_R, "com.example.b.Foo second", 14),
        ),
    },
    {
        "id": "9-rename-field",
        "title": "rename 字段（注释/字符串/局部 field 不误改）",
        "ref": req(APP_R, "this.field = 1;", 5),
        "new_name": "renamed",
        "expect": {
            "kind": "rename",
            "expect_edits": [
                (APP_R, "    int field;", 8, "renamed"),
                (APP_R, "this.field = 1;", 5, "renamed"),
                (APP_R, "int x = this.field;", 13, "renamed"),
            ],
            "unchanged": [
                (APP_R, "int field = 0;"),
                (APP_R, "// field in a comment"),
                (APP_R, '"field"'),
            ],
        },
    },
    {
        "id": "10-rename-method",
        "title": "rename 方法（调用/方法引用/重写点）",
        "ref": req(MODEL_R, "public void compute() {", 12),
        "new_name": "run",
        "expect": {
            "kind": "rename",
            "expect_edits": [
                (MODEL_R, "public void compute() {", 12, "run"),
                (USER_R, "model.compute();", 6, "run"),
                (APP_R, "model::compute", 7, "run"),
                (APP_R, "ref.compute();", 4, "run"),
                (CHILD_R, "public void compute() {", 12, "run"),
            ],
        },
    },
    {
        "id": "11-rename-class",
        "title": "rename 类（声明/import/new/泛型/extend）",
        "ref": req(MODEL_R, "public class Model {", 13),
        "new_name": "RenamedModel",
        "expect": {
            "kind": "rename",
            "expect_edits": [
                (MODEL_R, "public class Model {", 13, "RenamedModel"),
                (USER_R, "import com.example.base.Model;", 24, "RenamedModel"),
                (USER_R, "void use(Model model)", 9, "RenamedModel"),
                (USER2_R, "import com.example.base.Model;", 24, "RenamedModel"),
                (USER2_R, "void use(Model model)", 9, "RenamedModel"),
                (APP_R, "import com.example.base.Model;", 24, "RenamedModel"),
                (APP_R, "    Model model;", 4, "RenamedModel"),
                (APP_R, "Model m = new Model();", 0, "RenamedModel"),
                (APP_R, "new Model()", 4, "RenamedModel"),
                (APP_R, "Model ref = new Child();", 0, "RenamedModel"),
                (APP_R, "List<Model> models", 5, "RenamedModel"),
                (APP_R, "void qualify(Model other)", 13, "RenamedModel"),
                (CHILD_R, "import com.example.base.Model;", 24, "RenamedModel"),
                (CHILD_R, "extends Model", 8, "RenamedModel"),
            ],
            "old_token_absent_after": "Model",
        },
    },
    {
        "id": "12-rename-local",
        "title": "rename 局部变量（只改作用域内）",
        "ref": req(APP_R, "int x = 1;", 4),
        "new_name": "renamedX",
        "expect": {
            "kind": "rename",
            "expect_edits": [
                (APP_R, "int x = 1;", 4, "renamedX"),
                (APP_R, "int y = x;", 8, "renamedX"),
            ],
            "unchanged": [
                (APP_R, "int x = 2;"),
                (APP_R, "int y = x;"),
            ],
        },
    },
    {
        "id": "13a-prepare-field",
        "title": "prepareRename 字段 token 范围",
        "ref": req(APP_R, "this.field = 1;", 5),
        "expect": {
            "kind": "prepare",
            "expect_range": (APP_R, "this.field = 1;", 5, 5),
        },
    },
    {
        "id": "13b-prepare-local",
        "title": "prepareRename 局部变量 token 范围",
        "ref": req(APP_R, "int x = 1;", 4),
        "expect": {
            "kind": "prepare",
            "expect_range": (APP_R, "int x = 1;", 4, 1),
        },
    },
    {
        "id": "13c-prepare-class",
        "title": "prepareRename 类 token 范围",
        "ref": req(MODEL_R, "public class Model {", 13),
        "expect": {
            "kind": "prepare",
            "expect_range": (MODEL_R, "public class Model {", 13, 5),
        },
    },
    {
        "id": "13d-prepare-none",
        "title": "prepareRename 不可重命名处返回空",
        "ref": req(APP_R, "int field;", 0),
        "expect": {"kind": "prepare", "expect_range": None},
    },
    {
        "id": "14-rename-conflict",
        "title": "rename 冲突（新名与已有字段 model 冲突，记录约定）",
        "ref": req(APP_R, "this.field = 1;", 5),
        "new_name": "model",
        "expect": {"kind": "record"},
    },
]


def main():
    as_json = "--json" in sys.argv
    skip_open = "--no-open" in sys.argv
    for relative, text in FILES.items():
        path = PROJECT / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text.encode("utf-8"))
    if not EXE.is_file():
        print("missing " + str(EXE), file=sys.stderr)
        return 2

    def uri(relative):
        return (PROJECT / FULL.get(relative, relative)).as_uri()

    def uri_path(value):
        return Path(unquote(url2pathname(urlparse(value).path)))

    def apply_edits(texts, edits):
        """Apply {rel: [(start, end, new)]} to texts (descending offsets)."""
        for relative, items in edits.items():
            text = texts.get(relative, "")
            for start, end, new_text in sorted(items, key=lambda item: item[0], reverse=True):
                text = text[:start] + new_text + text[end:]
            texts[relative] = text
        return texts

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

    rows = []
    diagnostics = []
    try:
        send(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {"rootUri": PROJECT.as_uri()},
            }
        )
        read()
        deadline = time.time() + 30
        while time.time() < deadline:
            send({"jsonrpc": "2.0", "id": 100, "method": "ravac/indexStats", "params": {}})
            stats = read().get("result") or {}
            if not stats.get("building") and stats.get("generation", 0) > 0:
                break
            time.sleep(0.05)

        diagnostics = []
        if not skip_open:
            send(
                {
                    "jsonrpc": "2.0",
                    "method": "textDocument/didOpen",
                    "params": {
                        "textDocument": {
                            "uri": uri(MAIN_REL),
                            "languageId": "java",
                            "version": 1,
                            "text": APP,
                        }
                    },
                }
            )
            diagnostics = (read() or {}).get("params", {}).get("diagnostics", [])

        request_id = 10
        for scenario in SCENARIOS:
            ref_file, marker, offset = scenario["ref"]
            cursor = position(TEXTS[ref_file], marker, offset)
            expected = scenario["expect"]
            kind = expected["kind"]
            request_id += 2
            ok = False
            actual = ""
            if kind == "references":
                send(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "textDocument/references",
                        "params": {
                            "textDocument": {"uri": uri(ref_file)},
                            "position": cursor,
                            "context": {"includeDeclaration": True},
                        },
                    }
                )
                locations = (read() or {}).get("result") or []
                got = set()
                for location in locations:
                    relative = rel_of(uri_path(location["uri"]))
                    start = location["range"]["start"]
                    got.add((relative, start["line"], start["character"]))
                want = set()
                for item in expected["expect"]:
                    item_file, item_marker, item_offset = item
                    item_pos = position(TEXTS[item_file], item_marker, item_offset)
                    want.add((item_file, item_pos["line"], item_pos["character"]))
                ok = got == want
                actual = "n=" + str(len(got)) + " got=" + str(sorted(got))
                if not ok:
                    actual += " want=" + str(sorted(want))
            elif kind == "record":
                send(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "textDocument/rename",
                        "params": {
                            "textDocument": {"uri": uri(ref_file)},
                            "position": cursor,
                            "newName": scenario["new_name"],
                        },
                    }
                )
                result = (read() or {}).get("result")
                ok = True
                actual = json.dumps(result)[:200] if result else "null"
            elif kind == "rename":
                send(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "textDocument/rename",
                        "params": {
                            "textDocument": {"uri": uri(ref_file)},
                            "position": cursor,
                            "newName": scenario["new_name"],
                        },
                    }
                )
                result = (read() or {}).get("result")
                edits = {}
                got = set()
                if result:
                    for file_uri, items in (result.get("changes") or {}).items():
                        relative = rel_of(uri_path(file_uri))
                        text = TEXTS[relative]
                        for item in items:
                            start = item["range"]["start"]
                            end = item["range"]["end"]
                            start_offset = _offset(text, start)
                            end_offset = _offset(text, end)
                            edits.setdefault(relative, []).append(
                                (start_offset, end_offset, item["newText"])
                            )
                            got.add(
                                (
                                    relative,
                                    start["line"],
                                    start["character"],
                                    item["newText"],
                                )
                            )
                    want = set()
                    for item in expected["expect_edits"]:
                        item_file, item_marker, item_offset, new_text = item
                        item_pos = position(TEXTS[item_file], item_marker, item_offset)
                        want.add((item_file, item_pos["line"], item_pos["character"], new_text))
                    ok = got == want
                    actual = "n=" + str(len(got)) + " got=" + str(sorted(got))
                    if not ok:
                        actual += " want=" + str(sorted(want))
                    texts = apply_edits(dict(TEXTS), edits)
                    for item_file, needle in expected.get("unchanged", []):
                        if needle not in texts[item_file]:
                            ok = False
                            actual += " | changed " + needle
                    old = expected.get("old_token_absent_after")
                    if old:
                        import re

                        left = [
                            relative
                            for relative in TEXTS
                            if re.search(r"\b" + old + r"\b", texts[relative])
                        ]
                        if left:
                            ok = False
                            actual += " | old token left in " + str(left)
            else:
                send(
                    {
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "textDocument/prepareRename",
                        "params": {
                            "textDocument": {"uri": uri(ref_file)},
                            "position": cursor,
                        },
                    }
                )
                result = (read() or {}).get("result")
                expected_range = expected.get("expect_range")
                if expected_range is None:
                    ok = result is None
                    actual = "null" if result is None else json.dumps(result)[:120]
                else:
                    item_file, item_marker, item_offset, width = expected_range
                    item_pos = position(TEXTS[item_file], item_marker, item_offset)
                    want = (item_pos["line"], item_pos["character"], width)
                    got = None
                    if result and result.get("range"):
                        start = result["range"]["start"]
                        end = result["range"]["end"]
                        got = (start["line"], start["character"], end["character"] - start["character"])
                    ok = got == want
                    actual = str(got) + " want=" + str(want)
            rows.append(
                {
                    "id": scenario["id"],
                    "title": scenario["title"],
                    "ok": ok,
                    "actual": actual,
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


def _offset(text, position_object):
    line = position_object["line"]
    character = position_object["character"]
    current_line = 0
    offset = 0
    for ch in text:
        if current_line == line:
            break
        if ch == "\n":
            current_line += 1
        offset += 1
    return offset + character


if __name__ == "__main__":
    sys.exit(main())
