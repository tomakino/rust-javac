
"""Editor simulator: scenario assertions against the LSP server."""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from client import LspClient

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
CLASS_PATH = ROOT / "target" / "tmp" / "perf" / "javac"
WORK = ROOT / "target" / "tmp" / "editor-sim"

results = []


def check(name, condition, detail=""):
    results.append((name, bool(condition), detail))
    print(("PASS  " if condition else "FAIL  ") + name + (("  (" + detail + ")") if detail else ""))


def multi_file_diagnostics(client):
    a = WORK / "A.java"
    b = WORK / "B.java"
    client.did_open(a, "class A { void f() { int x = ; } }\n")
    diagnostics = client.wait_diagnostics(a)
    check("multi-file: broken file has one diagnostic", len(diagnostics) == 1, str(len(diagnostics)))

    client.did_open(b, "class B {}\n")
    diagnostics_b = client.wait_diagnostics(b)
    check("multi-file: clean file has none", len(diagnostics_b) == 0, str(len(diagnostics_b)))
    check(
        "multi-file: opening B keeps A's diagnostics",
        len(client.diagnostics.get(a.as_uri(), [])) == 1,
    )

    client.did_change(a, "class A {}\n", version=2)
    diagnostics = client.wait_diagnostics(a)
    check("multi-file: fixing A clears its diagnostics", len(diagnostics) == 0, str(len(diagnostics)))


def incremental_edits(client):
    a = WORK / "Incremental.java"
    broken = "class Incremental { void f() { int x = ; } }\n"
    fixed = "class Incremental { void f() { int x = 1; } }\n"
    client.did_open(a, broken)
    check("incremental: initial error", len(client.wait_diagnostics(a)) == 1)

    started = time.time()
    client.did_change(a, fixed, version=2)
    check("incremental: edit clears the error", len(client.wait_diagnostics(a)) == 0)
    elapsed = (time.time() - started) * 1000
    check("incremental: edit round-trip under 50ms", elapsed < 50, str(round(elapsed, 1)) + "ms")

    client.did_change(a, broken, version=3)
    check("incremental: error returns", len(client.wait_diagnostics(a)) == 1)
    client.did_change(a, fixed, version=4)
    check("incremental: second fix clears it", len(client.wait_diagnostics(a)) == 0)


def completion_isolation(client):
    a = WORK / "A.java"
    b = WORK / "B.java"
    client.did_open(a, "class A { int value; }\n")
    client.wait_diagnostics(a)
    client.did_open(b, "class B { }\n")
    client.wait_diagnostics(b)
    items = client.completion(b, 0, 10)
    labels = [item["label"] for item in items]
    check("completion: B lists its own class", "B" in labels, str(labels[:6]))
    check("completion: B does not leak A members", "value" not in labels, str(labels[:6]))


def member_completion(client):
    a = WORK / "MemberCompletion.java"
    text = "class MemberCompletion { int value; void f() { MemberCompletion m = new MemberCompletion(); m. } }\n"
    client.did_open(a, text)
    client.wait_diagnostics(a)
    column = text.index("m. }") + 2
    items = client.completion(a, 0, column)
    labels = [item["label"] for item in items]
    check("completion: member after dot", "value" in labels, str(labels[:10]))
    check("completion: method after dot", "f" in labels, str(labels[:10]))
    check("completion: dot hides unrelated classes", "MemberCompletion" not in labels, str(labels[:10]))
    check("completion: dot hides project classes", "A" not in labels, str(labels[:10]))


def hover_types(client):
    a = WORK / "HoverFoo.java"
    text = "class HoverFoo { int value; String name(int a) { return \"\"; } }\n"
    client.did_open(a, text)
    client.wait_diagnostics(a)

    value_column = text.index("value")
    value = client.hover(a, 0, value_column)
    value_text = ((value or {}).get("contents") or {}).get("value", "")
    check("hover: field type", "int" in value_text, value_text)

    name_column = text.index("name")
    name = client.hover(a, 0, name_column)
    name_text = ((name or {}).get("contents") or {}).get("value", "")
    check("hover: method return type", "String" in name_text, name_text)
    check("hover: method parameter type", "int" in name_text, name_text)


def definition_targets(client):
    a = WORK / "A.java"
    b = WORK / "B.java"
    client.did_open(a, "class A {}\n")
    client.wait_diagnostics(a)
    client.did_open(b, "class B { A a; }\n")
    client.wait_diagnostics(b)
    target = client.definition(b, 0, 10)
    uri = (target or {}).get("uri", "")
    check("definition: cross-file", target is not None and "A.java" in uri, uri)

    d = WORK / "D.java"
    text = "class D { int value; void f() { int x = value; } }\n"
    client.did_open(d, text)
    client.wait_diagnostics(d)
    target = client.definition(d, 0, text.index("value", 20))
    line = (target or {}).get("range", {}).get("start", {}).get("line")
    check("definition: same file", target is not None and line == 0, str(target))

    c = WORK / "C.java"
    list_text = "class C { java.util.List<String> list; }\n"
    client.did_open(c, list_text)
    client.wait_diagnostics(c)
    target = client.definition(c, 0, list_text.index("List"))
    uri = (target or {}).get("uri", "")
    check("definition: jdk source", target is not None and "List.java" in uri, uri)


def references_scope(client):
    path = WORK / "Ref.java"
    text = 'class Ref { int value; void f() { int x = value; /* value */ String s = "value"; } }\n'
    client.did_open(path, text)
    client.wait_diagnostics(path)
    refs = client.references(path, 0, text.index("value"))
    positions = [
        (ref["range"]["start"]["line"], ref["range"]["start"]["character"]) for ref in refs
    ]
    comment_column = text.index("/* value") + 3
    string_column = text.index('"value"') + 1
    check("references: declaration and use only", len(refs) == 2, str(positions))
    check(
        "references: comment ignored",
        all(column != comment_column for _, column in positions),
        str(positions),
    )
    check(
        "references: string literal ignored",
        all(column != string_column for _, column in positions),
        str(positions),
    )


def outline_and_lifecycle(client):
    path = WORK / "Outline.java"
    text = "class Outline { int value; void f() {} }\n"
    client.did_open(path, text)
    client.wait_diagnostics(path)
    symbols = client.document_symbols(path)
    check(
        "documentSymbol: class node",
        len(symbols) == 1 and symbols[0]["name"] == "Outline",
        str([s["name"] for s in symbols]),
    )
    children = symbols[0].get("children", []) if symbols else []
    names = [child["name"] for child in children]
    check("documentSymbol: field child", "value" in names, str(names))
    check("documentSymbol: method child", "f" in names, str(names))

    client.did_save(path, "class Outline {}\n")
    check("didSave: re-analyzes", len(client.wait_diagnostics(path)) == 0)
    client.did_close(path)
    check("didClose: clears diagnostics", len(client.wait_diagnostics(path)) == 0)


def rename_symbol(client):
    path = WORK / "Rename.java"
    text = "class Rename { int value; void f() { int x = value; } }\n"
    client.did_open(path, text)
    client.wait_diagnostics(path)
    result = client.rename(path, 0, text.index("value"), "count")
    edits = ((result or {}).get("changes") or {}).get(path.as_uri(), [])
    check("rename: edits every reference", len(edits) == 2, str(edits))
    check(
        "rename: new name applied",
        len(edits) == 2 and all(edit["newText"] == "count" for edit in edits),
        str(edits),
    )


def main():
    WORK.mkdir(parents=True, exist_ok=True)
    for path in WORK.glob("*.java"):
        path.unlink()
    client = LspClient(EXE, CLASS_PATH)
    try:
        client.initialize()
        multi_file_diagnostics(client)
        incremental_edits(client)
        completion_isolation(client)
        member_completion(client)
        hover_types(client)
        definition_targets(client)
        references_scope(client)
        outline_and_lifecycle(client)
        rename_symbol(client)
    finally:
        try:
            code = client.shutdown()
        except Exception:
            code = "error"
        check("server exits cleanly", code == 0, str(code))

    passed = sum(1 for _, ok, _ in results if ok)
    print("")
    print(str(passed) + "/" + str(len(results)) + " checks passed")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
