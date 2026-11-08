#!/usr/bin/env python3
"""X3: member-name positions for selection ranges.

`a.b` member names must select the name token itself, not an occurrence inside
the selected expression (`abc.b` must select the trailing `b`, not the `b` in
`abc`). Covers `a.b.c`, `this.x`, `super.x`.

Usage:
    py scripts/lsp/probe_x3_member_names.py
"""

import json
import shutil
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
BASE = ROOT / "target" / "tmp" / "x3-member-names" / ("run-" + str(os.getpid()))

TEXT = """package com.example;

class Base {
    int x;
    int y;
}

class B {
    int b;
    C c;
}

class C {
}

class A extends Base {
    B abc;

    void f() {
        int first = abc.b + abc.c.b;
        int second = this.x;
        int third = super.y;
    }
}
"""

PATH = "src/main/java/com/example/A.java"


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def lines_of(text: str):
    return text.split("\n")


def position_of(text: str, marker: str, occurrence: int = 0, offset: int = 0):
    start = -1
    for _ in range(occurrence + 1):
        start = text.index(marker, start + 1)
    start += offset
    prefix = text[:start]
    return prefix.count("\n"), start - (prefix.rfind("\n") + 1)


class Lsp:
    def __init__(self, root: Path):
        self.root = root
        self.proc = subprocess.Popen(
            [str(EXE)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        self.next_id = 100
        self.request("initialize", {"rootUri": root.as_uri()}, request_id=1)

    def send(self, message):
        body = json.dumps(message).encode()
        self.proc.stdin.write(b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)
        self.proc.stdin.flush()

    def read_until(self, request_id, timeout=20.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            headers = b""
            while b"\r\n\r\n" not in headers:
                chunk = self.proc.stdout.read(1)
                if not chunk:
                    return None
                headers += chunk
            length = int(headers.split(b"Content-Length: ")[1].split(b"\r\n")[0])
            message = json.loads(self.proc.stdout.read(length))
            if message.get("id") == request_id:
                return message
        return None

    def request(self, method, params, request_id=None):
        if request_id is None:
            self.next_id += 1
            request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        return (self.read_until(request_id) or {}).get("result")

    def selection(self, relative, position):
        return self.request(
            "textDocument/selectionRange",
            {
                "textDocument": {"uri": (self.root / relative).as_uri()},
                "positions": [{"line": position[0], "character": position[1]}],
            },
        )

    def close(self):
        try:
            self.proc.kill()
            self.proc.wait(timeout=5)
        except Exception:
            pass


def innermost(result):
    """LSP selectionRange: the first node is the smallest one; parents grow."""
    if not result:
        return None
    return result[0].get("range")


def main():
    root = BASE / "project"
    shutil.rmtree(BASE, ignore_errors=True)
    write(root / "pom.xml", "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId><artifactId>x3</artifactId><version>1.0</version></project>\n")
    write(root / PATH, TEXT)
    lsp = Lsp(root)
    rows = []

    def check(case_id, position, expected_text, expected_start):
        result = lsp.selection(PATH, position)
        range_ = innermost(result)
        text = lines_of(TEXT)[position[0]]
        actual = None
        if range_:
            start = (range_["start"]["line"], range_["start"]["character"])
            end = (range_["end"]["line"], range_["end"]["character"])
            actual = text[start[1] : end[1]] if start[0] == end[0] else "MULTILINE"
        ok = actual == expected_text and range_ is not None and range_["start"]["character"] == expected_start
        rows.append(
            {
                "id": case_id,
                "ok": ok,
                "actual": f"selected={actual!r} start={None if not range_ else range_['start']}",
            }
        )

    try:
        # `abc.b`: the member name `b` starts at column 14 (0-based).
        check(
            "a.b member name after a longer receiver",
            position_of(TEXT, "abc.b", 0, len("abc.")),
            "b",
            24,
        )
        # `a.b.c`: the last member name starts after `abc.c.`.
        check(
            "a.b.c last member name",
            position_of(TEXT, "abc.c.b", 0, len("abc.c.")),
            "b",
            34,
        )
        # `this.x`.
        check("this.x member name", position_of(TEXT, "this.x", 0, len("this.")), "x", 26)
        # `super.y`.
        check("super.y member name", position_of(TEXT, "super.y", 0, len("super.")), "y", 26)
    finally:
        lsp.close()

    failed = [row for row in rows if not row["ok"]]
    for row in rows:
        print(("PASS  " if row["ok"] else "FAIL  ") + row["id"])
        print("      " + row["actual"])
    print("")
    print(str(len(rows) - len(failed)) + "/" + str(len(rows)) + " cases passed")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
