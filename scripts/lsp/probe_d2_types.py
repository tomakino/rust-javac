#!/usr/bin/env python3
"""D2 probe: definition/hover for types, methods, constructors, refs, annotations.

Starts ``target/debug/ravac-lsp.exe``, initializes a small multi-package Maven
project, opens the main file through ``textDocument/didOpen`` and queries every
D2 scenario. Each row records the expectation and the actual answer.

Usage:
    py scripts/lsp/probe_d2_types.py [--json] [--no-open]
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
PROJECT = ROOT / "target" / "tmp" / ("d2-probe-project-" + str(os.getpid()))
MAIN_REL = "src/main/java/com/example/app/D2Probe.java"

BASE_TYPE = """package com.example.base;

public class BaseType {
    public static int inherited = 1;

    public BaseType() {
    }

    public BaseType(int value) {
    }

    public BaseType(String value) {
    }

    public static void staticMethod() {
    }

    public static void inheritedMethod() {
    }

    public void instanceMethod() {
    }

    public void over(int value) {
    }

    public void over(String value) {
    }
}
"""

BASE_IFACE = """package com.example.base;

public interface BaseIface {
    void run();
}
"""

COLOR = """package com.example.base;

public enum Color {
    RED,
    GREEN
}
"""

POINT = """package com.example.base;

public record Point(int x, int y) {
}
"""

MARKER = """package com.example.base;

public @interface Marker {
}
"""

ANNO = """package com.example.base;

public @interface Anno {
    int value();
}
"""

OUTER = """package com.example.base;

public class Outer {
    public static class Inner {
    }
}
"""

SUB = """package com.example.base;

public class Sub extends BaseType {
    public Sub() {
        super();
    }
}
"""

MAIN = """package com.example.app;

import com.example.base.Anno;
import com.example.base.BaseIface;
import com.example.base.BaseType;
import com.example.base.Color;
import com.example.base.Marker;
import com.example.base.Outer;
import com.example.base.Point;
import java.util.function.Supplier;
import static com.example.base.Sub.inherited;
import static com.example.base.Sub.inheritedMethod;
import static java.util.Collections.*;

public class D2Probe extends BaseType {
    public D2Probe() {
        super();
    }

    void types() {
        BaseType baseType = null;
        BaseIface baseIface = null;
        Color color = null;
        Point point = null;
        Outer.Inner inner = null;
        java.util.List<String> list = null;
    }

    void anonymous() {
        Runnable runnable = new Runnable() {
            public void run() {
            }
        };
    }

    void localClass() {
        class Local {
        }
        Local local = new Local();
    }

    void overloads() {
        over(1);
        over("x");
    }

    void overrides() {
        this.instanceMethod();
    }

    @Override
    public void instanceMethod() {
    }

    void constructors() {
        new BaseType(1);
        new BaseType("x");
    }

    void references() {
        Runnable staticRef = BaseType::staticMethod;
        Supplier<BaseType> constructorRef = BaseType::new;
        BaseType base = new BaseType();
        Runnable boundRef = base::instanceMethod;
    }

    void annotationUse() {
        @Marker
        int markerField = 0;
        @Anno(value = 1)
        int annoField = 0;
    }

    void generics() {
        Box<String> box = null;
        String value = box.get();
    }

    void wildcardImport() {
        emptyList();
    }

    void inheritedImport() {
        int value = inherited;
        inheritedMethod();
    }
}

class Box<T> {
    T value;

    T get() {
        return value;
    }
}
"""

FILES = {
    "pom.xml": (
        '<project><modelVersion>4.0.0</modelVersion>'
        "<groupId>com.example</groupId><artifactId>d2-probe</artifactId>"
        "<version>1.0</version></project>\n"
    ),
    "src/main/java/com/example/base/BaseType.java": BASE_TYPE,
    "src/main/java/com/example/base/BaseIface.java": BASE_IFACE,
    "src/main/java/com/example/base/Color.java": COLOR,
    "src/main/java/com/example/base/Point.java": POINT,
    "src/main/java/com/example/base/Marker.java": MARKER,
    "src/main/java/com/example/base/Anno.java": ANNO,
    "src/main/java/com/example/base/Outer.java": OUTER,
    "src/main/java/com/example/base/Sub.java": SUB,
    MAIN_REL: MAIN,
}

BASE = "src/main/java/com/example/base/"


def position(text, marker, offset):
    """LSP position of `marker` + `offset` characters (markers must be unique)."""
    start = text.index(marker) + offset
    prefix = text[:start]
    return {
        "line": prefix.count("\n"),
        "character": start - (prefix.rfind("\n") + 1),
    }


SCENARIOS = [
    {
        "id": "1a-class-reference",
        "title": "类引用（跨包）",
        "ref": (MAIN_REL, "BaseType baseType", 0),
        "def": (BASE + "BaseType.java", "public class BaseType {", len("public class ")),
        "hover": ["BaseType"],
    },
    {
        "id": "1b-interface-reference",
        "title": "接口引用（跨包）",
        "ref": (MAIN_REL, "BaseIface baseIface", 0),
        "def": (
            BASE + "BaseIface.java",
            "public interface BaseIface {",
            len("public interface "),
        ),
        "hover": ["BaseIface"],
    },
    {
        "id": "1c-enum-reference",
        "title": "枚举类型引用",
        "ref": (MAIN_REL, "Color color", 0),
        "def": (BASE + "Color.java", "public enum Color {", len("public enum ")),
        "hover": ["Color"],
    },
    {
        "id": "1d-record-reference",
        "title": "record 类型引用",
        "ref": (MAIN_REL, "Point point", 0),
        "def": (
            BASE + "Point.java",
            "public record Point(int x, int y) {",
            len("public record "),
        ),
        "hover": ["Point"],
    },
    {
        "id": "2-nested-class",
        "title": "嵌套类 Outer.Inner",
        "ref": (MAIN_REL, "Outer.Inner inner", len("Outer.")),
        "def": (BASE + "Outer.java", "public static class Inner {", len("public static class ")),
        "hover": ["Inner"],
    },
    {
        "id": "3-anonymous-class",
        "title": "匿名类 new Runnable(){}",
        "ref": (MAIN_REL, "new Runnable()", len("new ")),
        "def": {"jdk_suffix": "Runnable.java"},
        "hover": ["Runnable"],
    },
    {
        "id": "4-local-class",
        "title": "局部类引用",
        "ref": (MAIN_REL, "Local local = new Local();", 0),
        "def": (MAIN_REL, "class Local {", len("class ")),
        "hover": ["Local"],
    },
    {
        "id": "5a-overload-int",
        "title": "方法重载 over(1)",
        "ref": (MAIN_REL, "over(1);", 0),
        "def": (
            BASE + "BaseType.java",
            "public void over(int value) {",
            len("public void "),
        ),
        "hover": ["over", "int"],
    },
    {
        "id": "5b-overload-string",
        "title": "方法重载 over(\"x\")",
        "ref": (MAIN_REL, 'over("x");', 0),
        "def": (
            BASE + "BaseType.java",
            "public void over(String value) {",
            len("public void "),
        ),
        "hover": ["over", "String"],
    },
    {
        "id": "6a-override-declaration",
        "title": "重写方法声明（hover Overrides）",
        "ref": (MAIN_REL, "public void instanceMethod() {", len("public void ")),
        "def": (MAIN_REL, "public void instanceMethod() {", len("public void ")),
        "hover": ["Overrides", "instanceMethod"],
    },
    {
        "id": "6b-override-call",
        "title": "重写方法的调用 → 子类 override",
        "ref": (MAIN_REL, "this.instanceMethod();", len("this.")),
        "def": (MAIN_REL, "public void instanceMethod() {", len("public void ")),
        "hover": ["instanceMethod"],
    },
    {
        "id": "7a-constructor-int",
        "title": "构造器 new BaseType(1)",
        "ref": (MAIN_REL, "new BaseType(1);", len("new ")),
        "def": (
            BASE + "BaseType.java",
            "public BaseType(int value) {",
            len("public "),
        ),
        "hover": ["BaseType"],
    },
    {
        "id": "7b-constructor-string",
        "title": "构造器重载 new BaseType(\"x\")",
        "ref": (MAIN_REL, 'new BaseType("x");', len("new ")),
        "def": (
            BASE + "BaseType.java",
            "public BaseType(String value) {",
            len("public "),
        ),
        "hover": ["BaseType"],
    },
    {
        "id": "8a-method-ref-static",
        "title": "方法引用 A::method",
        "ref": (MAIN_REL, "BaseType::staticMethod", len("BaseType::")),
        "def": (
            BASE + "BaseType.java",
            "public static void staticMethod() {",
            len("public static void "),
        ),
        "hover": ["staticMethod"],
    },
    {
        "id": "8b-method-ref-constructor",
        "title": "构造器引用 A::new",
        "ref": (MAIN_REL, "BaseType::new", len("BaseType::")),
        "def": (BASE + "BaseType.java", "public BaseType() {", len("public ")),
        "hover": ["BaseType"],
    },
    {
        "id": "8c-method-ref-bound",
        "title": "实例方法引用 obj::method",
        "ref": (MAIN_REL, "base::instanceMethod", len("base::")),
        "def": (
            BASE + "BaseType.java",
            "public void instanceMethod() {",
            len("public void "),
        ),
        "hover": ["instanceMethod"],
    },
    {
        "id": "9-annotation-type",
        "title": "注解类型 @Marker",
        "ref": (MAIN_REL, "@Marker", 1),
        "def": (
            BASE + "Marker.java",
            "public @interface Marker {",
            len("public @interface "),
        ),
        "hover": ["Marker"],
    },
    {
        "id": "10-annotation-member",
        "title": "注解成员 value",
        "ref": (MAIN_REL, "value = 1", 0),
        "def": (BASE + "Anno.java", "int value();", len("int ")),
        "hover": ["value"],
    },
    {
        "id": "11-generic-method-signature",
        "title": "方法签名中的类型参数 T",
        "ref": (MAIN_REL, "T get() {", 0),
        "def": (MAIN_REL, "class Box<T>", len("class Box<")),
        "hover": ["T"],
    },
    {
        "id": "12-qualified-jdk-type",
        "title": "全限定名 java.util.List",
        "ref": (MAIN_REL, "java.util.List<String>", len("java.util.")),
        "def": {"jdk_suffix": "List.java"},
        "hover": ["List"],
    },
    {
        "id": "13-static-import-wildcard",
        "title": "静态导入通配符后 emptyList",
        "ref": (MAIN_REL, "emptyList();", 0),
        "def": {"jdk_suffix": "Collections.java"},
        "hover": ["emptyList"],
    },
    {
        "id": "14-inherited-static-import",
        "title": "继承的静态成员 import static Sub.inherited",
        "ref": (MAIN_REL, "Sub.inherited;", len("Sub.")),
        "def": (
            BASE + "BaseType.java",
            "public static int inherited = 1;",
            len("public static int "),
        ),
        "hover": ["inherited"],
    },
    {
        "id": "14b-inherited-static-use",
        "title": "继承静态成员的使用处",
        "ref": (MAIN_REL, "int value = inherited;", len("int value = ")),
        "def": (
            BASE + "BaseType.java",
            "public static int inherited = 1;",
            len("public static int "),
        ),
        "hover": ["inherited"],
    },
]


def main():
    as_json = "--json" in sys.argv
    skip_open = "--no-open" in sys.argv
    PROJECT.mkdir(parents=True, exist_ok=True)
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
                            "text": MAIN,
                        }
                    },
                }
            )
            diagnostics = (read() or {}).get("params", {}).get("diagnostics", [])

        request_id = 10
        for scenario in SCENARIOS:
            ref_file, marker, offset = scenario["ref"]
            cursor = position(FILES[ref_file], marker, offset)
            request_id += 2
            send(
                {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "method": "textDocument/hover",
                    "params": {
                        "textDocument": {"uri": uri(ref_file)},
                        "position": cursor,
                    },
                }
            )
            hover_text = (((read() or {}).get("result") or {}).get("contents") or {}).get(
                "value", ""
            )
            hover_ok = bool(hover_text) and all(
                needle in hover_text for needle in scenario["hover"]
            )

            send(
                {
                    "jsonrpc": "2.0",
                    "id": request_id + 1,
                    "method": "textDocument/definition",
                    "params": {
                        "textDocument": {"uri": uri(ref_file)},
                        "position": cursor,
                    },
                }
            )
            definition = (read() or {}).get("result")
            definition_ok = False
            actual = "null"
            if definition:
                target = Path(unquote(url2pathname(urlparse(definition["uri"]).path)))
                start = definition.get("range", {}).get("start", {})
                actual = str(target) + ":" + str(start.get("line")) + ":" + str(
                    start.get("character")
                )
                if isinstance(scenario["def"], dict):
                    definition_ok = str(target).lower().endswith(
                        scenario["def"]["jdk_suffix"].lower()
                    )
                else:
                    def_file, def_marker, def_offset = scenario["def"]
                    expected = position(FILES[def_file], def_marker, def_offset)
                    definition_ok = (
                        target.name == Path(def_file).name
                        and start.get("line") == expected["line"]
                        and start.get("character") == expected["character"]
                    )
                    actual += (
                        " (expected "
                        + Path(def_file).name
                        + ":"
                        + str(expected["line"])
                        + ":"
                        + str(expected["character"])
                        + ")"
                    )
            rows.append(
                {
                    "id": scenario["id"],
                    "title": scenario["title"],
                    "hover_ok": hover_ok,
                    "hover": hover_text.replace("\n", " | ")[:140],
                    "definition_ok": definition_ok,
                    "definition": actual,
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
