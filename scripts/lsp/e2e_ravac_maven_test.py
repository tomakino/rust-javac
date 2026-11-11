#!/usr/bin/env python3
"""End-to-end LSP verification against the real Gson 2.10.1 Maven project.

Project: C:\\Users\\Lin\\IdeaProjects\\ravac-maven-test (81 main sources, release 17).
The script only reads project files; in-memory overlays are used for probes that
need modified text and are closed afterwards.

Usage:
    py scripts/lsp/e2e_ravac_maven_test.py [--project PATH]
"""

import argparse
import json
import queue
import subprocess
import sys
import threading
import time
from pathlib import Path
from urllib.parse import unquote, urlparse
from urllib.request import url2pathname

ROOT_REPO = Path(__file__).resolve().parents[2]
EXE = ROOT_REPO / "target" / "debug" / "ravac-lsp.exe"
DEFAULT_PROJECT = Path(r"C:\Users\Lin\IdeaProjects\ravac-maven-test")

GSON_PATH = "src/main/java/com/google/gson/Gson.java"
JSON_ELEMENT_PATH = "src/main/java/com/google/gson/JsonElement.java"
TYPE_ADAPTER_PATH = "src/main/java/com/google/gson/TypeAdapter.java"
GSON_BUILDER_PATH = "src/main/java/com/google/gson/GsonBuilder.java"


def read_text(root: Path, relative: str) -> str:
    return (root / relative).read_text(encoding="utf-8")


def position_of(text: str, marker: str, offset: int = 0, occurrence: int = 0):
    start = -1
    for _ in range(occurrence + 1):
        start = text.index(marker, start + 1)
    start += offset
    prefix = text[:start]
    return prefix.count("\n"), start - (prefix.rfind("\n") + 1)


def end_of(text: str, marker: str, offset: int = 0, occurrence: int = 0):
    line, column = position_of(text, marker, offset, occurrence)
    return line, column + len(marker) - offset


def rel_of(location) -> str:
    if not location or not location.get("uri"):
        return ""
    return unquote(url2pathname(urlparse(location["uri"]).path)).replace("\\", "/")


def contains_range(outer, inner) -> bool:
    if not outer or not inner:
        return False

    def before(left, right):
        return (left["line"], left["character"]) <= (right["line"], right["character"])

    return before(outer["start"], inner["start"]) and before(inner["end"], outer["end"])


class Lsp:
    def __init__(self, root: Path):
        self.root = root
        self.proc = None
        self.responses: "queue.Queue[dict]" = queue.Queue()
        self.stderr_lines: list[str] = []
        self.diagnostics: dict[str, list] = {}
        self.diag_condition = threading.Condition()
        self.next_id = 100
        self.capabilities = {}
        self.initialize_ms = 0.0
        self.index_ms = 0.0
        self.index_stats = {}

    def start(self):
        self.proc = subprocess.Popen(
            [str(EXE)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        threading.Thread(target=self._reader, daemon=True).start()
        threading.Thread(target=self._stderr_reader, daemon=True).start()
        started = time.time()
        result = self.request(
            "initialize",
            {
                "rootUri": self.root.as_uri(),
                "capabilities": {
                    "textDocument": {
                        "synchronization": {"didSave": True},
                        "codeLens": {"dynamicRegistration": False},
                    },
                    "workspace": {"workspaceFolders": True},
                },
                "workspaceFolders": [{"uri": self.root.as_uri(), "name": self.root.name}],
            },
            request_id=1,
        )
        self.initialize_ms = (time.time() - started) * 1000
        self.capabilities = (result or {}).get("capabilities") or {}
        self.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
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
            message = json.loads(self.proc.stdout.read(length))
            if "id" in message:
                self.responses.put(message)
            elif message.get("method") == "textDocument/publishDiagnostics":
                params = message.get("params") or {}
                with self.diag_condition:
                    self.diagnostics[params.get("uri", "")] = params.get("diagnostics") or []
                    self.diag_condition.notify_all()

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

    def notify(self, method, params):
        self.send({"jsonrpc": "2.0", "method": method, "params": params})

    def read_until(self, request_id, timeout=60.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                message = self.responses.get(timeout=max(0.0, deadline - time.time()))
            except queue.Empty:
                return None
            if message.get("id") == request_id:
                return message
        return None

    def request(self, method, params, request_id=None, timeout=60.0):
        if request_id is None:
            self.next_id += 1
            request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        message = self.read_until(request_id, timeout=timeout)
        if message is None:
            return None
        if "error" in message:
            raise RuntimeError(f"{method}: {message['error']}")
        return message.get("result")

    def uri(self, relative: str) -> str:
        return (self.root / relative).as_uri()

    def wait_ready(self):
        deadline = time.time() + 20
        while time.time() < deadline:
            stats = self.request("ravac/artifactStats", {}) or {}
            if stats.get("status") != "building":
                break
            time.sleep(0.02)
        started = time.time()
        deadline = time.time() + 20
        while time.time() < deadline:
            self.request(
                "textDocument/documentSymbol",
                {"textDocument": {"uri": self.uri(GSON_PATH)}},
            )
            stats = self.request("ravac/indexStats", {}) or {}
            if not stats.get("building") and int(stats.get("generation") or 0) > 0:
                self.index_stats = stats
                self.index_ms = (time.time() - started) * 1000
                return
            time.sleep(0.02)
        self.index_stats = self.request("ravac/indexStats", {}) or {}
        self.index_ms = (time.time() - started) * 1000

    def did_open(self, relative: str, text: str):
        self.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": self.uri(relative),
                    "languageId": "java",
                    "version": 1,
                    "text": text,
                }
            },
        )

    def did_close(self, relative: str):
        self.notify("textDocument/didClose", {"textDocument": {"uri": self.uri(relative)}})

    def wait_diagnostics(self, relative: str, timeout=15.0):
        uri = self.uri(relative)
        deadline = time.time() + timeout
        with self.diag_condition:
            while uri not in self.diagnostics and time.time() < deadline:
                self.diag_condition.wait(timeout=max(0.0, deadline - time.time()))
            return self.diagnostics.get(uri)

    def wait_all_diagnostics(self, relatives, timeout=90.0):
        uris = {self.uri(relative) for relative in relatives}
        deadline = time.time() + timeout
        with self.diag_condition:
            while not uris.issubset(self.diagnostics.keys()) and time.time() < deadline:
                self.diag_condition.wait(timeout=max(0.0, deadline - time.time()))
        return {uri: self.diagnostics.get(uri) for uri in uris}

    def close(self):
        try:
            if self.proc and self.proc.poll() is None:
                self.proc.kill()
                self.proc.wait(timeout=5)
        except Exception:
            pass


class Report:
    def __init__(self):
        self.rows = []

    def add(self, item, name, ok, detail, ms):
        self.rows.append({"item": item, "name": name, "ok": bool(ok), "detail": detail, "ms": ms})
        print(
            f"{'PASS' if ok else 'FAIL'}  {item:>2} {name:<26} {ms:8.1f}ms  {detail}",
            flush=True,
        )

    def summary(self):
        failed = [row for row in self.rows if not row["ok"]]
        print("", flush=True)
        print(f"{len(self.rows) - len(failed)}/{len(self.rows)} checks passed", flush=True)
        return 0 if not failed else 1


def safe(report, item, name, fn):
    started = time.time()
    try:
        ok, detail = fn()
    except Exception as error:  # noqa: BLE001 - report every probe failure
        ok, detail = False, f"EXCEPTION {type(error).__name__}: {error}"
    report.add(item, name, ok, detail, (time.time() - started) * 1000)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", default=str(DEFAULT_PROJECT))
    args = parser.parse_args()
    root = Path(args.project)
    if not (root / "pom.xml").exists():
        print(f"project not found: {root}", file=sys.stderr)
        return 2

    report = Report()
    lsp = Lsp(root)
    lsp.start()
    try:
        print(
            f"initialize={lsp.initialize_ms:.1f}ms index={lsp.index_ms:.1f}ms "
            f"stats={lsp.index_stats}",
            flush=True,
        )

        gson_text = read_text(root, GSON_PATH)
        json_element_text = read_text(root, JSON_ELEMENT_PATH)
        builder_text = read_text(root, GSON_BUILDER_PATH)

        # 1. index
        def item_index():
            stats = lsp.index_stats
            files = int(stats.get("files") or 0)
            symbols = int(stats.get("symbols") or 0)
            ok = files >= 81 and lsp.index_ms <= 3000
            return ok, f"files={files} symbols={symbols} index={lsp.index_ms:.0f}ms"

        safe(report, 1, "index", item_index)

        # 2. documentSymbol
        def item_document_symbol():
            details = []
            ok = True
            for relative in (GSON_PATH, JSON_ELEMENT_PATH):
                symbols = lsp.request(
                    "textDocument/documentSymbol",
                    {"textDocument": {"uri": lsp.uri(relative)}},
                ) or []
                flat = []

                def walk(items):
                    for item in items:
                        flat.append(item)
                        walk(item.get("children") or [])

                walk(symbols)
                valid = all(
                    item.get("name")
                    and 1 <= int(item.get("kind") or 0) <= 26
                    and contains_range(item.get("range"), item.get("selectionRange"))
                    for item in flat
                )
                details.append(f"{Path(relative).name}:{len(flat)}")
                ok = ok and len(flat) > 0 and valid
            return ok, " ".join(details)

        safe(report, 2, "documentSymbol", item_document_symbol)

        # positions
        to_json = position_of(gson_text, "public String toJson(Object src)")
        to_json = (to_json[0], to_json[1] + len("public String "))
        field = position_of(
            gson_text, "private final ConcurrentMap<TypeToken<?>, TypeAdapter<?>> typeTokenCache"
        )
        field = (field[0], field[1] + len("private final ConcurrentMap<TypeToken<?>, TypeAdapter<?>> "))
        class_name = position_of(gson_text, "public final class Gson")
        class_name = (class_name[0], class_name[1] + len("public final class "))
        import_project = position_of(gson_text, "import com.google.gson.stream.JsonReader;")
        import_project = (import_project[0], import_project[1] + len("import "))
        import_jdk = position_of(gson_text, "import java.io.IOException;")
        import_jdk = (import_jdk[0], import_jdk[1] + len("import "))
        new_gson_in_builder = position_of(builder_text, "new Gson(", occurrence=1, offset=len("new "))
        body_expr = position_of(gson_text, "return toJson(src, src.getClass());", offset=len("return toJson(src, "))
        new_json_reader = position_of(
            gson_text,
            "public JsonReader newJsonReader(Reader reader)",
            offset=len("public JsonReader "),
        )
        new_json_writer = position_of(
            gson_text,
            "public JsonWriter newJsonWriter(Writer writer)",
            offset=len("public JsonWriter "),
        )
        to_json_typed = position_of(gson_text, "public String toJson(Object src, Type typeOfSrc)")
        to_json_typed = (to_json_typed[0], to_json_typed[1] + len("public String "))
        from_json_typed = position_of(
            gson_text,
            "public <T> T fromJson(JsonReader reader, Type typeOfT)",
            offset=len("public <T> T "),
        )
        gson_to_string = position_of(
            gson_text,
            "public String toString()",
            offset=len("public String "),
        )
        type_token_get = position_of(gson_text, "TypeToken.get(typeOfSrc)", offset=len("TypeToken."))

        # 3. definition
        def item_definition():
            cases = []
            results = []
            for name, relative, position in [
                ("method-call", GSON_PATH, type_token_get),
                ("field", GSON_PATH, position_of(gson_text, "typeTokenCache", occurrence=1)),
                ("import-project", GSON_PATH, import_project),
                ("import-jdk", GSON_PATH, import_jdk),
                ("new-ctor", GSON_BUILDER_PATH, position_of(builder_text, "new Gson(", occurrence=1, offset=len("new "))),
            ]:
                location = lsp.request(
                    "textDocument/definition",
                    {
                        "textDocument": {"uri": lsp.uri(relative)},
                        "position": {"line": position[0], "character": position[1]},
                    },
                )
                target = rel_of(location if isinstance(location, dict) else None)
                results.append(f"{name}->{Path(target).name if target else 'null'}")
                if name == "import-project":
                    cases.append(target.endswith("JsonReader.java"))
                elif name == "import-jdk":
                    cases.append(target.endswith("IOException.java"))
                elif name == "method-call":
                    cases.append(target.endswith("TypeToken.java"))
                elif name == "new-ctor":
                    cases.append(target.endswith("Gson.java"))
                else:
                    cases.append(target.endswith("Gson.java"))
            return all(cases), " ".join(results)

        safe(report, 3, "definition", item_definition)

        # 4. hover
        def item_hover():
            positions = [
                ("field", GSON_PATH, position_of(gson_text, "typeTokenCache", occurrence=1)),
                ("method", GSON_PATH, to_json),
                ("class", GSON_PATH, class_name),
                ("local", GSON_PATH, body_expr),
            ]
            details = []
            ok = True
            for name, relative, position in positions:
                hover = lsp.request(
                    "textDocument/hover",
                    {
                        "textDocument": {"uri": lsp.uri(relative)},
                        "position": {"line": position[0], "character": position[1]},
                    },
                )
                value = ((hover or {}).get("contents") or {}).get("value") or ""
                details.append(f"{name}:{len(value)}")
                ok = ok and len(value.strip()) > 0
            signature_hover = lsp.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": new_json_reader[0], "character": new_json_reader[1]},
                },
            )
            signature_value = ((signature_hover or {}).get("contents") or {}).get("value") or ""
            signature_ok = (
                "JsonReader newJsonReader(Reader reader)" in signature_value
                and "(Reader)JsonReader" not in signature_value
            )
            details.append("newJsonReader=" + ("ok" if signature_ok else repr(signature_value[:80])))
            ok = ok and signature_ok
            writer_hover = lsp.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": new_json_writer[0], "character": new_json_writer[1]},
                },
            )
            writer_value = ((writer_hover or {}).get("contents") or {}).get("value") or ""
            writer_ok = (
                "JsonWriter newJsonWriter(Writer writer)" in writer_value
                and "Returns a new JSON writer configured" in writer_value
                and "- `GsonBuilder#disableHtmlEscaping()`" in writer_value
                and "<ul>" not in writer_value
            )
            details.append("newJsonWriter-doc=" + ("ok" if writer_ok else repr(writer_value[:80])))
            ok = ok and writer_ok
            from_json_hover = lsp.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": from_json_typed[0], "character": from_json_typed[1]},
                },
            )
            from_json_value = ((from_json_hover or {}).get("contents") or {}).get("value") or ""
            from_json_paragraphs = [
                "Reads the next JSON value from `reader`",
                "\n\nSince `Type` is not parameterized by T",
                "\n\nUnlike the other `fromJson` methods",
                "\n\nThe JSON data is parsed in lenient mode",
                "- **reader** the reader whose next JSON value should be deserialized",
            ]
            from_json_ok = all(
                paragraph in from_json_value for paragraph in from_json_paragraphs
            ) and "<p>" not in from_json_value
            details.append(
                "fromJson-paragraphs=" + ("ok" if from_json_ok else repr(from_json_value[:120]))
            )
            ok = ok and from_json_ok
            to_string_hover = lsp.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": gson_to_string[0], "character": gson_to_string[1]},
                },
            )
            to_string_value = ((to_string_hover or {}).get("contents") or {}).get("value") or ""
            to_string_ok = all(
                part in to_string_value
                for part in [
                    "com.google.gson.Gson",
                    "java.lang.Object",
                    "public String toString()",
                    "Returns a string representation of the object.",
                    "Inherited from java.lang.Object#toString()",
                    "**Overrides:**",
                    "java.lang.Object#toString()](file://",
                    "*Source:",
                ]
            )
            details.append(
                "toString-inherited=" + ("ok" if to_string_ok else repr(to_string_value[:120]))
            )
            link_start = to_string_value.find("](file://")
            uri_ok = False
            if link_start >= 0:
                link_end = to_string_value.find(")", link_start)
                if link_end > link_start:
                    link_uri = to_string_value[link_start + 2 : link_end]
                    parsed = urlparse(link_uri)
                    uri_ok = parsed.scheme == "file" and (parsed.fragment or "").startswith("L")
            details.append("toString-uri=" + ("ok" if uri_ok else "bad"))
            code_ok = (
                "```\ngetClass().getName() + '@' + Integer.toHexString(hashCode())\n```"
                in to_string_value
                and "> ```" not in to_string_value
            )
            details.append("toString-code=" + ("ok" if code_ok else "bad"))
            tags_ok = all(
                label in to_string_value
                for label in [
                    "**API Note**",
                    "**Implementation Requirements**",
                    "**Returns**",
                ]
            ) and "**@apiNote**" not in to_string_value
            details.append("toString-tags=" + ("ok" if tags_ok else "bad"))
            snippet_relative = "src/main/java/com/google/gson/HoverSnippetProbe.java"
            snippet_text = (
                "package com.google.gson;\n"
                "class HoverSnippetProbe {\n"
                "    /**\n"
                "     * Snippet body.\n"
                "     * @implSpec\n"
                "     * {@snippet lang=java :\n"
                "     *     int snippetValue = 1;\n"
                "     * }\n"
                "     * The {@link java.util.Objects#toIdentityString(Object) Objects.toIdentityString}\n"
                "     * method returns the string for an object.\n"
                "     * <dl>\n"
                "     * <dt>Alpha</dt><dd>First definition.</dd>\n"
                "     * <dt>Beta</dt><dd>Second definition.</dd>\n"
                "     * </dl>\n"
                "     */\n"
                "    public void probe() {\n"
                "    }\n"
                "}\n"
            )
            snippet_position = position_of(
                snippet_text, "public void probe()", offset=len("public void ")
            )
            lsp.did_open(snippet_relative, snippet_text)
            snippet_hover = lsp.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": lsp.uri(snippet_relative)},
                    "position": {
                        "line": snippet_position[0],
                        "character": snippet_position[1],
                    },
                },
            )
            lsp.did_close(snippet_relative)
            snippet_value = ((snippet_hover or {}).get("contents") or {}).get("value") or ""
            snippet_ok = (
                "```java" in snippet_value
                and "int snippetValue = 1;" in snippet_value
                and "**Implementation Requirements**" in snippet_value
                and "toIdentityString" in snippet_value
                and "method returns the string for an object" in snippet_value
                and "- **Alpha** — First definition.\n- **Beta** — Second definition."
                in snippet_value
            )
            details.append("snippet=" + ("ok" if snippet_ok else repr(snippet_value[:100])))
            ok = ok and to_string_ok and uri_ok and tags_ok and code_ok and snippet_ok
            return ok, " ".join(details)

        safe(report, 4, "hover", item_hover)

        # 5. references
        def item_references():
            locations = lsp.request(
                "textDocument/references",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": to_json_typed[0], "character": to_json_typed[1]},
                    "context": {"includeDeclaration": True},
                },
            ) or []
            valid = all(rel_of(loc).endswith(".java") and loc.get("range") for loc in locations)
            detail = f"count={len(locations)}"
            if locations:
                detail += f" first={Path(rel_of(locations[0])).name}:{locations[0]['range']['start']['line']}"
            return len(locations) >= 2 and valid, detail

        safe(report, 5, "references", item_references)

        # 6. rename / prepareRename
        def item_rename():
            prepare = lsp.request(
                "textDocument/prepareRename",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": field[0], "character": field[1]},
                },
            )
            prepared_text = ""
            prepared_range = (prepare or {}).get("range")
            if prepared_range:
                start = prepared_range["start"]
                prepared_text = gson_text.splitlines()[start["line"]][
                    start["character"]: start["character"] + (prepared_range["end"]["character"] - start["character"])
                ]
            edit = lsp.request(
                "textDocument/rename",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": field[0], "character": field[1]},
                    "newName": "typeTokenCacheProbe",
                },
            ) or {}
            changes = edit.get("changes") or {}
            edits = sum(len(value) for value in changes.values())
            ok = prepared_text == "typeTokenCache" and edits >= 2 and bool(changes)
            return ok, f"prepare={prepared_text!r} files={len(changes)} edits={edits}"

        safe(report, 6, "rename", item_rename)

        # 7. completion
        completion_path = "src/main/java/com/google/gson/ProbeCompletion.java"
        completion_text = """package com.google.gson;

class ProbeCompletion {
    void f() {
        Gson gson = new Gson();
        gson.
    }
}
"""
        import_probe_path = "src/main/java/com/google/gson/internal/ProbeImport.java"
        import_probe_text = """package com.google.gson.internal;

class ProbeImport {
    JsonEl
}
"""

        def item_completion():
            lsp.did_open(completion_path, completion_text)
            lsp.did_open(import_probe_path, import_probe_text)
            member_position = position_of(completion_text, "gson.", offset=len("gson."))
            members = lsp.request(
                "textDocument/completion",
                {
                    "textDocument": {"uri": lsp.uri(completion_path)},
                    "position": {"line": member_position[0], "character": member_position[1]},
                    "context": {"triggerKind": 1},
                },
            )
            member_items = members if isinstance(members, list) else (members or {}).get("items") or []
            labels = [item.get("label") for item in member_items]
            member_ok = any(label in ("toJson", "fromJson", "toJsonTree") for label in labels)
            import_position = position_of(import_probe_text, "JsonEl", offset=len("JsonEl"))
            imports = lsp.request(
                "textDocument/completion",
                {
                    "textDocument": {"uri": lsp.uri(import_probe_path)},
                    "position": {"line": import_position[0], "character": import_position[1]},
                    "context": {"triggerKind": 1},
                },
            )
            import_items = imports if isinstance(imports, list) else (imports or {}).get("items") or []
            import_labels = [item.get("label") for item in import_items]
            import_ok = "JsonElement" in import_labels
            lsp.did_close(completion_path)
            lsp.did_close(import_probe_path)
            return member_ok and import_ok, (
                f"members={len(labels)} has-toJson={member_ok} "
                f"imports={len(import_labels)} has-JsonElement={import_ok}"
            )

        safe(report, 7, "completion", item_completion)

        # 8. signatureHelp
        signature_path = "src/main/java/com/google/gson/ProbeSignature.java"
        signature_text = """package com.google.gson;

class ProbeSignature {
    void f() {
        Gson gson = new Gson();
        gson.toJson(
    }
}
"""

        def item_signature():
            lsp.did_open(signature_path, signature_text)
            position = position_of(signature_text, "gson.toJson(", offset=len("gson.toJson("))
            help = lsp.request(
                "textDocument/signatureHelp",
                {
                    "textDocument": {"uri": lsp.uri(signature_path)},
                    "position": {"line": position[0], "character": position[1]},
                },
            )
            signatures = (help or {}).get("signatures") or []
            labels = [signature.get("label") or "" for signature in signatures]
            lsp.did_close(signature_path)
            return len(signatures) >= 1 and all(labels), f"signatures={len(signatures)} {labels[:3]}"

        safe(report, 8, "signatureHelp", item_signature)

        # 9. semanticTokens
        def item_semantic_tokens():
            result = lsp.request(
                "textDocument/semanticTokens/full",
                {"textDocument": {"uri": lsp.uri(GSON_PATH)}},
            ) or {}
            data = result.get("data") or []
            legend = (((lsp.capabilities or {}).get("semanticTokensProvider") or {}).get("legend") or {}).get(
                "tokenTypes"
            ) or []
            type_count = len(legend)
            valid = len(data) % 5 == 0 and type_count > 0
            if valid:
                for index in range(0, len(data), 5):
                    if not (0 <= data[index + 3] < type_count):
                        valid = False
                        break
            return len(data) > 0 and valid, f"tokens={len(data)//5} legend={type_count}"

        safe(report, 9, "semanticTokens", item_semantic_tokens)

        # 10. codeLens
        def item_code_lens():
            lenses = lsp.request(
                "textDocument/codeLens",
                {"textDocument": {"uri": lsp.uri(GSON_PATH)}},
            ) or []
            keys = set()
            duplicates = 0
            references = 0
            for lens in lenses:
                command = lens.get("command") or {}
                key = (
                    lens.get("range", {}).get("start", {}).get("line"),
                    lens.get("range", {}).get("start", {}).get("character"),
                    command.get("command"),
                    command.get("title"),
                )
                if key in keys:
                    duplicates += 1
                keys.add(key)
                title = command.get("title") or ""
                if "references" in title or "implementations" in title:
                    references += 1
            return len(lenses) > 0 and duplicates == 0 and references > 0, (
                f"lenses={len(lenses)} duplicates={duplicates} counts={references}"
            )

        safe(report, 10, "codeLens", item_code_lens)

        # 11. inlayHints
        def item_inlay_hints():
            details = []
            total = 0
            for relative in (GSON_PATH, JSON_ELEMENT_PATH, GSON_BUILDER_PATH):
                hints = lsp.request(
                    "textDocument/inlayHint",
                    {"textDocument": {"uri": lsp.uri(relative)}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 4000, "character": 0}}},
                ) or []
                total += len(hints)
                details.append(f"{Path(relative).name}:{len(hints)}")
            var_tokens = sum(
                1
                for path in (root / "src" / "main" / "java").rglob("*.java")
                for line in path.read_text(encoding="utf-8", errors="replace").splitlines()
                if " var " in line
            )
            return total > 0, f"{' '.join(details)} var-lines={var_tokens}"

        safe(report, 11, "inlayHints", item_inlay_hints)

        # 12. foldingRange
        def item_folding():
            folds = lsp.request(
                "textDocument/foldingRange",
                {"textDocument": {"uri": lsp.uri(GSON_PATH)}},
            ) or []
            imports_fold = any(
                int(fold.get("startLine") or 0) <= 16 and int(fold.get("endLine") or 0) >= 40
                for fold in folds
            )
            class_fold = any((int(fold.get("endLine") or 0) - int(fold.get("startLine") or 0)) > 50 for fold in folds)
            return len(folds) >= 5 and (imports_fold or class_fold), (
                f"folds={len(folds)} imports={imports_fold} class={class_fold}"
            )

        safe(report, 12, "foldingRange", item_folding)

        # 13. selectionRange
        def item_selection():
            position = (body_expr[0], body_expr[1] + len("src.getCl"))
            result = lsp.request(
                "textDocument/selectionRange",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "positions": [{"line": position[0], "character": position[1]}],
                },
            ) or []
            chain = []
            node = result[0] if result else None
            while node:
                chain.append(node.get("range"))
                node = node.get("parent")
            nested = all(
                contains_range(chain[index + 1], chain[index]) for index in range(len(chain) - 1)
            )
            return len(chain) >= 4 and nested, f"levels={len(chain)} nested={nested}"

        safe(report, 13, "selectionRange", item_selection)

        # 14. documentLink
        def item_document_link():
            links = lsp.request(
                "textDocument/documentLink",
                {"textDocument": {"uri": lsp.uri(GSON_PATH)}},
            ) or []
            targets = [link.get("target") or "" for link in links]
            project = any(target.endswith("JsonReader.java") for target in targets)
            jdk = any(target.endswith("IOException.java") for target in targets)
            return len(links) > 0 and project and jdk, (
                f"links={len(links)} project={project} jdk={jdk}"
            )

        safe(report, 14, "documentLink", item_document_link)

        # 15. codeAction (add import / organize imports / generate)
        action_path = "src/main/java/com/google/gson/internal/ProbeAction.java"
        action_text = """package com.google.gson.internal;

class ProbeAction {
    JsonElement e;
}
"""
        organize_path = "src/main/java/com/google/gson/internal/ProbeOrganize.java"
        organize_text = """package com.google.gson.internal;

import java.util.Map;
import java.util.List;

class ProbeOrganize {
    List<String> names;
}
"""
        generate_path = "src/main/java/com/google/gson/internal/ProbeGenerate.java"
        generate_text = """package com.google.gson.internal;

class ProbeGenerate {
    private int value;
}
"""

        def item_code_action():
            lsp.did_open(action_path, action_text)
            position = position_of(action_text, "JsonElement", offset=2)
            actions = lsp.request(
                "textDocument/codeAction",
                {
                    "textDocument": {"uri": lsp.uri(action_path)},
                    "range": {"start": {"line": position[0], "character": position[1]}, "end": {"line": position[0], "character": position[1] + 1}},
                    "context": {"diagnostics": []},
                },
            ) or []
            add_import = any(
                "import" in (action.get("title") or "").lower()
                and any(
                    "import com.google.gson.JsonElement" in edit.get("newText", "")
                    for change_list in ((action.get("edit") or {}).get("changes") or {}).values()
                    for edit in change_list
                )
                for action in actions
            )
            lsp.did_close(action_path)

            lsp.did_open(organize_path, organize_text)
            edits = lsp.request(
                "workspace/executeCommand",
                {"command": "ravac.organizeImports", "arguments": [lsp.uri(organize_path)]},
            ) or []
            organize_ok = isinstance(edits, list) and len(edits) >= 1
            lsp.did_close(organize_path)

            lsp.did_open(generate_path, generate_text)
            class_position = position_of(generate_text, "ProbeGenerate") 
            generate_actions = lsp.request(
                "textDocument/codeAction",
                {
                    "textDocument": {"uri": lsp.uri(generate_path)},
                    "range": {"start": {"line": class_position[0], "character": class_position[1]}, "end": {"line": class_position[0], "character": class_position[1] + len("ProbeGenerate")}},
                    "context": {"diagnostics": []},
                },
            ) or []
            generate_ok = any("generate" in (action.get("title") or "").lower() for action in generate_actions)
            lsp.did_close(generate_path)
            return add_import and organize_ok and generate_ok, (
                f"add-import={add_import} organize={len(edits)} generate={len(generate_actions)}"
            )

        safe(report, 15, "codeAction", item_code_action)

        # 17. typeDefinition (before diagnostics; diagnostics last)
        def item_type_definition():
            position = position_of(gson_text, "TypeToken.get(typeOfSrc)", offset=len("TypeToken.get("))
            location = lsp.request(
                "textDocument/typeDefinition",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": position[0], "character": position[1]},
                },
            )
            target = rel_of(location if isinstance(location, dict) else None)
            return bool(target) and target.endswith(".java"), f"->{Path(target).name if target else 'null'}"

        safe(report, 17, "typeDefinition", item_type_definition)

        # 18. implementation
        def item_implementation():
            adapter_text = read_text(root, TYPE_ADAPTER_PATH)
            position = position_of(adapter_text, "public abstract class TypeAdapter")
            position = (position[0], position[1] + len("public abstract class "))
            locations = lsp.request(
                "textDocument/implementation",
                {
                    "textDocument": {"uri": lsp.uri(TYPE_ADAPTER_PATH)},
                    "position": {"line": position[0], "character": position[1]},
                },
            ) or []
            names = [Path(rel_of(location)).name for location in locations]
            return len(locations) > 0, f"count={len(locations)} files={sorted(set(names))[:5]}"

        safe(report, 18, "implementation", item_implementation)

        # 19. workspace/symbol
        def item_workspace_symbol():
            symbols = lsp.request("workspace/symbol", {"query": "Gson"}) or []
            names = [symbol.get("name") or "" for symbol in symbols]
            return len(symbols) > 0 and any("Gson" in name for name in names), (
                f"count={len(symbols)} first={names[:5]}"
            )

        safe(report, 19, "workspaceSymbol", item_workspace_symbol)

        # 20. formatting / onTypeFormatting
        def item_formatting():
            lsp.did_open(GSON_PATH, gson_text)
            position = position_of(gson_text, "return toJson(src, src.getClass());")
            formatted = lsp.request(
                "textDocument/formatting",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "options": {"tabSize": 2, "insertSpaces": True},
                },
            )
            on_type = lsp.request(
                "textDocument/onTypeFormatting",
                {
                    "textDocument": {"uri": lsp.uri(GSON_PATH)},
                    "position": {"line": position[0], "character": position[1] + 30},
                    "ch": ";",
                    "options": {"tabSize": 2, "insertSpaces": True},
                },
            )
            lsp.did_close(GSON_PATH)
            format_ok = formatted is None or isinstance(formatted, list)
            on_type_ok = on_type is None or isinstance(on_type, list)
            jar = ROOT_REPO / "target" / "tools" / "google-java-format.jar"
            note = f"format={'list' if isinstance(formatted, list) else 'null'} onType={'list' if isinstance(on_type, list) else 'null'}"
            if formatted is None:
                note += f" format-jar={jar.exists()}"
            return format_ok and on_type_ok, note

        safe(report, 20, "formatting", item_formatting)

        # 16. diagnostics for every project file (open all, then wait)
        files = [
            path.relative_to(root).as_posix()
            for path in sorted((root / "src" / "main" / "java").rglob("*.java"))
        ]

        def item_diagnostics():
            for relative in files:
                lsp.did_open(relative, read_text(root, relative))
            published = lsp.wait_all_diagnostics(files, timeout=240)
            errors = []
            for relative in files:
                uri = lsp.uri(relative)
                for diagnostic in published.get(uri) or []:
                    if int(diagnostic.get("severity") or 0) == 1:
                        errors.append(
                            f"{relative}:{diagnostic.get('range', {}).get('start', {}).get('line')}: "
                            f"{(diagnostic.get('message') or '')[:80]}"
                        )
            for relative in files:
                lsp.did_close(relative)
            return not errors, f"files={len(files)} errors={len(errors)}" + (
                " " + " | ".join(errors[:3]) if errors else ""
            )

        safe(report, 16, "diagnostics", item_diagnostics)

        code = report.summary()
        if lsp.stderr_lines:
            interesting = [
                line
                for line in lsp.stderr_lines
                if "panic" in line.lower() or "error" in line.lower()
            ]
            if interesting:
                print("stderr:", interesting[-5:], flush=True)
        return code
    finally:
        lsp.close()


if __name__ == "__main__":
    sys.exit(main())
