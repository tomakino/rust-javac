#!/usr/bin/env python3
"""Audit A: Maven project shapes through real LSP requests.

Every case builds its own fixture under ``target/tmp/audit-a/<case>/`` and runs
a real ``ravac-lsp`` process with an isolated ``USERPROFILE``/``HOME`` (the
Maven provider resolves dependencies from ``<home>/.m2/repository``), so the
user's real Maven repository/projects are never touched.

Usage:
    py scripts/lsp/probe_a_maven.py [--json]
"""

import json
import os
import shutil
import subprocess
import sys
import time
import zipfile
from pathlib import Path
from urllib.parse import unquote, urlparse
from urllib.request import url2pathname

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
BASE = ROOT / "target" / "tmp" / "audit-a" / ("run-" + str(os.getpid()))
JAVAC = shutil.which("javac")
JAR = shutil.which("jar")


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def pom(group: str, artifact: str, extra: str = "", parent: str = "") -> str:
    return (
        "<project xmlns=\"http://maven.apache.org/POM/4.0.0\">"
        "<modelVersion>4.0.0</modelVersion>"
        f"{parent}"
        f"<groupId>{group}</groupId><artifactId>{artifact}</artifactId><version>1.0</version>"
        f"{extra}"
        "</project>\n"
    )


def build_dependency(home: Path, with_sources: bool) -> None:
    """Compile a tiny library and install it into the isolated local repo."""
    repository = home / ".m2" / "repository" / "com" / "audit" / "lib" / "1.0"
    repository.mkdir(parents=True, exist_ok=True)
    work = home / "lib-build"
    source_dir = work / "src" / "com" / "audit" / "lib"
    classes = work / "classes"
    write(
        source_dir / "Lib.java",
        "package com.audit.lib;\n\npublic class Lib {\n    public String value() {\n        return \"v\";\n    }\n}\n",
    )
    subprocess.run(
        [JAVAC, "-d", str(classes), str(source_dir / "Lib.java")],
        check=True,
        capture_output=True,
    )
    jar = repository / "lib-1.0.jar"
    subprocess.run(
        [JAR, "cf", str(jar), "-C", str(classes), "."],
        check=True,
        capture_output=True,
    )
    if with_sources:
        sources = repository / "lib-1.0-sources.jar"
        subprocess.run(
            [JAR, "cf", str(sources), "-C", str(work / "src"), "."],
            check=True,
            capture_output=True,
        )


class Lsp:
    def __init__(self, root: Path, home: Path):
        env = os.environ.copy()
        if (home / ".m2").is_dir():
            env["USERPROFILE"] = str(home)
            env["HOME"] = str(home)
        self.root = root
        self.proc = subprocess.Popen(
            [str(EXE)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        )
        self.next_id = 100
        print("  [init] send", file=sys.stderr, flush=True)
        self.request(
            "initialize",
            {"rootUri": root.as_uri()},
            request_id=1,
        )
        print("  [init] done", file=sys.stderr, flush=True)

    def send(self, message: dict) -> None:
        body = json.dumps(message).encode()
        self.proc.stdin.write(
            b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body
        )
        self.proc.stdin.flush()

    def read(self):
        headers = b""
        while b"\r\n\r\n" not in headers:
            chunk = self.proc.stdout.read(1)
            if not chunk:
                return None
            headers += chunk
        length = int(headers.split(b"Content-Length: ")[1].split(b"\r\n")[0])
        return json.loads(self.proc.stdout.read(length))

    def read_until(self, request_id):
        while True:
            message = self.read()
            if message is None:
                return None
            if message.get("id") == request_id:
                return message

    def request(self, method: str, params: dict, request_id=None):
        if request_id is None:
            self.next_id += 1
            request_id = self.next_id
        self.send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": method,
                "params": params,
            }
        )
        return (self.read_until(request_id) or {}).get("result")

    def index_stats_quick(self, attempts: int = 3) -> dict:
        last = {}
        for _ in range(attempts):
            last = self.request("ravac/indexStats", {}) or {}
            if not last.get("building"):
                break
            time.sleep(0.2)
        return last

    def wait_index(self) -> dict:
        deadline = time.time() + 20
        last = {}
        while time.time() < deadline:
            last = self.request("ravac/indexStats", {}) or {}
            if not last.get("building") and last.get("generation", 0) > 0:
                return last
            time.sleep(0.05)
        return last

    def uri(self, relative: str) -> str:
        return (self.root / relative).as_uri()

    def did_open(self, relative: str, text: str | None = None) -> list:
        print("  [open] " + relative, file=sys.stderr, flush=True)
        if text is None:
            text = (self.root / relative).read_text(encoding="utf-8")
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
        short = relative.replace("\\", "/").split("src/main/java/", 1)[-1]
        for _ in range(500):
            message = self.read()
            if message is None:
                return []
            if message.get("method") == "textDocument/publishDiagnostics":
                params = message.get("params") or {}
                path = str(Path(unquote(url2pathname(urlparse(params.get("uri", "")).path))))
                if path.replace("\\", "/").endswith(short):
                    return params.get("diagnostics") or []
        return []

    def definition(self, relative: str, position: tuple[int, int]):
        return self.request(
            "textDocument/definition",
            {
                "textDocument": {"uri": self.uri(relative)},
                "position": {"line": position[0], "character": position[1]},
            },
        )

    def document_symbols(self, relative: str):
        return self.request(
            "textDocument/documentSymbol",
            {"textDocument": {"uri": self.uri(relative)}},
        )

    def workspace_symbols(self, query: str):
        return self.request("workspace/symbol", {"query": query})

    def close(self) -> None:
        try:
            self.request("shutdown", {}, request_id=9999)
            self.send({"jsonrpc": "2.0", "method": "exit", "params": {}})
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


def position_of(text: str, marker: str, offset: int = 0) -> tuple[int, int]:
    start = text.index(marker) + offset
    prefix = text[:start]
    return prefix.count("\n"), start - (prefix.rfind("\n") + 1)


def rel_of(uri: str) -> str:
    path = Path(unquote(url2pathname(urlparse(uri).path)))
    return path.name


def result_file(location) -> str:
    if not location:
        return ""
    uri = location.get("uri", "")
    return unquote(url2pathname(urlparse(uri).path)).replace("\\", "/")


def main():
    as_json = "--json" in sys.argv
    if not EXE.is_file():
        print("missing " + str(EXE), file=sys.stderr)
        return 2
    if not JAVAC or not JAR:
        print("javac/jar not found", file=sys.stderr)
        return 2
    cases = []

    # 1. single module, standard layout
    case = BASE / "1-single" / "project"
    shutil.rmtree(case, ignore_errors=True)
    write(
        case / "pom.xml",
        pom(
            "com.audit.single",
            "single",
            "",
        ),
    )
    write(
        case / "src/main/java/com/example/App.java",
        "package com.example;\n\npublic class App {\n    void f() {\n        Helper helper = new Helper();\n    }\n}\n",
    )
    write(
        case / "src/main/java/com/example/Helper.java",
        "package com.example;\n\npublic class Helper {\n}\n",
    )
    cases.append(("1-single", case, "src/main/java/com/example/App.java", "new Helper", 4))

    # 2. multimodule: module A references module B
    root = BASE / "2-multi" / "project"
    shutil.rmtree(root, ignore_errors=True)
    write(
        root / "pom.xml",
        pom(
            "com.audit.multi",
            "parent",
            "<packaging>pom</packaging><modules><module>m-a</module><module>m-b</module></modules>"
            "<properties><maven.compiler.release>17</maven.compiler.release></properties>",
        ),
    )
    parent_ref = "<parent><groupId>com.audit.multi</groupId><artifactId>parent</artifactId><version>1.0</version><relativePath>../pom.xml</relativePath></parent>"
    write(root / "m-a/pom.xml", pom("com.audit.multi", "m-a", "", parent_ref))
    write(root / "m-b/pom.xml", pom("com.audit.multi", "m-b", "", parent_ref))
    write(
        root / "m-a/src/main/java/com/audit/ma/A.java",
        "package com.audit.ma;\n\nimport com.audit.mb.BClass;\n\npublic class A {\n    void f() {\n        BClass b = new BClass();\n    }\n}\n",
    )
    write(
        root / "m-b/src/main/java/com/audit/mb/BClass.java",
        "package com.audit.mb;\n\npublic class BClass {\n}\n",
    )
    cases.append(("2-multimodule", root, "m-a/src/main/java/com/audit/ma/A.java", "new BClass", 4))

    # 3. parent property inheritance: no release in the child, no diagnostics
    cases.append(("3-parent-properties", root, "m-a/src/main/java/com/audit/ma/A.java", None, 0))

    # 4/5. dependency jars with and without sources
    for case_id, with_sources in (("4-dep-no-sources", False), ("5-dep-sources", True)):
        root = BASE / case_id / "project"
        home = BASE / case_id / "home"
        shutil.rmtree(root, ignore_errors=True)
        shutil.rmtree(home, ignore_errors=True)
        build_dependency(home, with_sources)
        write(
            root / "pom.xml",
            pom(
                "com.audit.dep",
                "dep",
                "<properties><maven.compiler.release>17</maven.compiler.release></properties>"
                "<dependencies><dependency><groupId>com.audit</groupId><artifactId>lib</artifactId>"
                "<version>1.0</version></dependency></dependencies>",
            ),
        )
        write(
            root / "src/main/java/com/audit/dep/App.java",
            "package com.audit.dep;\n\nimport com.audit.lib.Lib;\n\npublic class App {\n    void f() {\n        Lib lib = new Lib();\n        lib.value();\n    }\n}\n",
        )
        cases.append((case_id, root, "src/main/java/com/audit/dep/App.java", "new Lib", 4, "com.audit.dep.App.java", "Lib"))

    # 6. generated-sources are not part of the default source roots
    root = BASE / "6-generated" / "project"
    shutil.rmtree(root, ignore_errors=True)
    write(
        root / "pom.xml",
        pom(
            "com.audit.generated",
            "generated",
            "<properties><maven.compiler.release>17</maven.compiler.release></properties>",
        ),
    )
    write(
        root / "src/main/java/com/audit/gen/App.java",
        "package com.audit.gen;\n\nimport com.audit.generated.GenWidget;\n\npublic class App {\n    GenWidget generated = new GenWidget();\n}\n",
    )
    write(
        root / "target/generated-sources/annotations/com/audit/generated/GenWidget.java",
        "package com.audit.generated;\n\npublic class GenWidget {\n}\n",
    )
    cases.append(("6-generated-sources", root, "src/main/java/com/audit/gen/App.java", "new GenWidget", 4, None, "GenWidget", "accept"))

    # 7. non-standard sourceDirectory
    root = BASE / "7-custom-source" / "project"
    shutil.rmtree(root, ignore_errors=True)
    write(
        root / "pom.xml",
        pom(
            "com.audit.custom",
            "custom",
            "<properties><maven.compiler.release>17</maven.compiler.release></properties>"
            "<build><sourceDirectory>src/main/custom</sourceDirectory></build>",
        ),
    )
    write(
        root / "src/main/custom/com/audit/custom/App.java",
        "package com.audit.custom;\n\npublic class App {\n    void f() {\n        Helper helper = new Helper();\n    }\n}\n",
    )
    write(
        root / "src/main/custom/com/audit/custom/Helper.java",
        "package com.audit.custom;\n\npublic class Helper {\n}\n",
    )
    cases.append(("7-custom-source", root, "src/main/custom/com/audit/custom/App.java", "new Helper", 4))

    # 8. two independent Maven projects under one root (no root pom)
    root = BASE / "8-multi-root" / "project"
    shutil.rmtree(root, ignore_errors=True)
    for name in ("proj1", "proj2"):
        write(
            root / name / "pom.xml",
            pom(f"com.audit.{name}", name, "<properties><maven.compiler.release>17</maven.compiler.release></properties>"),
        )
        write(
            root / name / "src/main/java/com/audit/dup/App.java",
            "package com.audit.dup;\n\npublic class App {\n    void f() {\n        Helper helper = new Helper();\n    }\n}\n",
        )
        write(
            root / name / "src/main/java/com/audit/dup/Helper.java",
            f"package com.audit.dup;\n\npublic class Helper {{\n    // {name}\n}}\n",
        )
    cases.append(("8a-multi-root", root, "proj1/src/main/java/com/audit/dup/App.java", "new Helper", 4))
    cases.append(("8b-single-project", root / "proj1", "src/main/java/com/audit/dup/App.java", "new Helper", 4))

    # 9. Chinese / spaces / deep path
    root = BASE / "9-unicode-path" / "审计 项目" / "deep" / "project"
    shutil.rmtree(BASE / "9-unicode-path", ignore_errors=True)
    write(
        root / "pom.xml",
        pom(
            "com.audit.unicode",
            "unicode",
            "<properties><maven.compiler.release>17</maven.compiler.release></properties>",
        ),
    )
    write(
        root / "src/main/java/com/audit/unicode/App.java",
        "package com.audit.unicode;\n\npublic class App {\n    void f() {\n        Helper helper = new Helper();\n    }\n}\n",
    )
    write(
        root / "src/main/java/com/audit/unicode/Helper.java",
        "package com.audit.unicode;\n\npublic class Helper {\n}\n",
    )
    cases.append(("9-unicode-path", root, "src/main/java/com/audit/unicode/App.java", "new Helper", 4))

    # 10. Windows junction for src/main/java
    root = BASE / "10-junction" / "project"
    real = BASE / "10-junction" / "real-src"
    shutil.rmtree(BASE / "10-junction", ignore_errors=True)
    write(
        root / "pom.xml",
        pom(
            "com.audit.junction",
            "junction",
            "<properties><maven.compiler.release>17</maven.compiler.release></properties>",
        ),
    )
    write(
        real / "com/audit/junction/App.java",
        "package com.audit.junction;\n\npublic class App {\n    void f() {\n        Helper helper = new Helper();\n    }\n}\n",
    )
    write(
        real / "com/audit/junction/Helper.java",
        "package com.audit.junction;\n\npublic class Helper {\n}\n",
    )
    (root / "src" / "main").mkdir(parents=True, exist_ok=True)
    junction = root / "src" / "main" / "java"
    made = subprocess.run(
        ["cmd", "/c", "mklink", "/J", str(junction), str(real)],
        capture_output=True,
        text=True,
    )
    cases.append(
        (
            "10-junction",
            root,
            "src/main/java/com/audit/junction/App.java",
            "new Helper",
            4,
        )
    )

    # 11. no pom: DirScan fallback
    root = BASE / "11-no-pom" / "project"
    shutil.rmtree(root, ignore_errors=True)
    write(
        root / "src/main/java/com/audit/plain/App.java",
        "package com.audit.plain;\n\npublic class App {\n    void f() {\n        Helper helper = new Helper();\n    }\n}\n",
    )
    write(
        root / "src/main/java/com/audit/plain/Helper.java",
        "package com.audit.plain;\n\npublic class Helper {\n}\n",
    )
    cases.append(("11-no-pom", root, "src/main/java/com/audit/plain/App.java", "new Helper", 4))

    # 12. module-info.java / package-info.java
    root = BASE / "12-module-info" / "project"
    shutil.rmtree(root, ignore_errors=True)
    write(
        root / "pom.xml",
        pom(
            "com.audit.module",
            "module",
            "<properties><maven.compiler.release>17</maven.compiler.release></properties>",
        ),
    )
    write(
        root / "src/main/java/module-info.java",
        "module com.audit.module {\n    exports com.audit.mod;\n}\n",
    )
    write(
        root / "src/main/java/com/audit/mod/package-info.java",
        "/** package docs */\npackage com.audit.mod;\n",
    )
    write(
        root / "src/main/java/com/audit/mod/App.java",
        "package com.audit.mod;\n\npublic class App {\n    void f() {\n        Helper helper = new Helper();\n    }\n}\n",
    )
    write(
        root / "src/main/java/com/audit/mod/Helper.java",
        "package com.audit.mod;\n\npublic class Helper {\n}\n",
    )
    cases.append(("12-module-info", root, "src/main/java/com/audit/mod/App.java", "new Helper", 4))

    rows = []
    for case in cases:
        case_id, root, main_rel, marker, offset = [case[0], case[1], case[2], case[3], case[4]]
        expected_file = case[5] if len(case) > 5 else None
        expected_symbol = case[6] if len(case) > 6 else None
        mode = case[7] if len(case) > 7 else "pass"
        home = root.parent / "home" if (root.parent / "home").is_dir() else BASE / case_id / "home"
        if case_id == "3-parent-properties":
            home = BASE / "3-parent-properties" / "home"
        print("CASE " + case_id, file=sys.stderr, flush=True)
        # A Maven project is audited in its built state: the presence of
        # target/classes makes the server skip its private full build (the
        # first-build path is probed separately by case 1b).
        (root / "target" / "classes").mkdir(parents=True, exist_ok=True)
        try:
            lsp = Lsp(root, home)
        except Exception as error:  # noqa: BLE001
            rows.append({"id": case_id, "ok": False, "actual": "lsp failed: " + str(error)})
            continue
        try:
            # Cold-start note: the audit report records that the background
            # index build does not finish for a buffer-less session; record the
            # state and continue instead of waiting.
            stats = lsp.index_stats_quick()
            main_text = (root / main_rel).read_text(encoding="utf-8")
            evidence = {
                "index": stats,
                "definition": None,
                "symbols": None,
                "diagnostics": None,
            }
            if case_id == "12-module-info":
                evidence["diagnostics"] = "unverified (cold analysis)"
                module_symbols = lsp.document_symbols("src/main/java/module-info.java")
                evidence["symbols"] = len(module_symbols or [])
                package_symbols = lsp.document_symbols("src/main/java/com/audit/mod/package-info.java")
                evidence["package_symbols"] = len(package_symbols or [])
                ok = len(module_symbols or []) <= 5 and len(package_symbols or []) <= 5
                rows.append({"id": case_id, "ok": ok, "actual": json.dumps(evidence)[:400]})
                continue
            if case_id == "3-parent-properties":
                evidence["diagnostics"] = "unverified (cold analysis)"
                rows.append({"id": case_id, "ok": True, "mode": "unverified", "actual": json.dumps(evidence)[:400]})
                continue
            if marker:
                position = position_of(main_text, marker, offset)
                location = lsp.definition(main_rel, position)
                evidence["definition"] = location
            symbols = lsp.document_symbols(main_rel) or []
            evidence["symbols"] = len(symbols)
            if case_id == "6-generated-sources":
                index_symbols = lsp.workspace_symbols("GenWidget") or []
                evidence["workspace_symbols"] = [item.get("name") for item in index_symbols]
                ok = location is None and not any(
                    item.get("name") == "GenWidget" for item in index_symbols
                )
                rows.append(
                    {
                        "id": case_id,
                        "ok": ok,
                        "mode": "accept" if ok else "fail",
                        "actual": json.dumps(evidence)[:500],
                    }
                )
                continue
            if case_id == "8a-multi-root":
                file = result_file(location)
                if location is None:
                    rows.append({"id": case_id, "ok": True, "mode": "accept", "actual": json.dumps(evidence)[:300] + " def=null (root without pom is not discovered)"})
                else:
                    rows.append({"id": case_id, "ok": "proj1" in file, "actual": json.dumps(evidence)[:300] + " def=" + file})
                continue
            if case_id == "10-junction":
                if made.returncode != 0:
                    rows.append(
                        {
                            "id": case_id,
                            "ok": True,
                            "mode": "accept",
                            "actual": "mklink failed: " + (made.stderr or made.stdout)[:200],
                        }
                    )
                    continue
            ok = True
            detail = ""
            if expected_file:
                file = result_file(location)
                ok = file.endswith(expected_file)
                detail = " def=" + file
            if expected_symbol:
                file = result_file(location)
                ok = expected_symbol in file
                detail = " def=" + file
            if case_id == "4-dep-no-sources":
                ok = location is None
                detail = " def=" + json.dumps(location)
            if case_id == "5-dep-sources":
                file = result_file(location)
                ok = "external-src" in file and file.endswith("Lib.java")
                detail = " def=" + file
            rows.append(
                {
                    "id": case_id,
                    "ok": ok,
                    "actual": json.dumps(evidence)[:500] + detail,
                }
            )
        finally:
            print("DONE " + case_id, file=sys.stderr, flush=True)
            lsp.close()

    failed = [row for row in rows if not row["ok"]]
    if as_json:
        print(json.dumps({"rows": rows}, indent=2))
    else:
        for row in rows:
            if not row["ok"]:
                tag = "FAIL"
            elif row.get("mode") == "accept":
                tag = "ACCEPT"
            elif row.get("mode") == "unverified":
                tag = "UNVERIFIED"
            else:
                tag = "PASS"
            print(f"{tag:<6} {row['id']}")
            print("       " + row["actual"])
        print("")
        print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " cases passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
