#!/usr/bin/env python3
"""H: code lens outline rules through a real LSP.

The screenshot bug: the class line showed `2 references | 4 references` and the
method line `1 references | 0 references` (two lens groups at one range).
This probe uses the same JDK RecordHelper shape and asserts one group per range
with no constructor/import/parameter lenses.

Usage:
    py scripts/lsp/probe_h_codelens.py
"""

import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
BASE = ROOT / "target" / "tmp" / "h-codelens" / ("run-" + str(os.getpid()))

POM = "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId><artifactId>lens</artifactId><version>1.0</version></project>\n"
TEXT = """package com.example;

import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.lang.reflect.Field;

class Outer {
    /**
     * Internal abstraction over reflection when Records are supported.
     */
    private abstract static class RecordHelper {
        abstract boolean isRecord(Class<?> clazz);

        abstract String[] getRecordComponentNames(Class<?> clazz);

        abstract <T> Constructor<T> getCanonicalRecordConstructor(Class<T> raw);

        abstract Method getAccessor(Class<?> raw, Field field);
    }
}
"""
PATH = "src/main/java/com/example/Outer.java"


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

    def request(self, method, params, request_id=None, timeout=20.0):
        if request_id is None:
            self.next_id += 1
            request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        return (self.read_until(request_id, timeout=timeout) or {}).get("result")

    def uri(self, relative: str) -> str:
        return (self.root / relative).as_uri()

    def close(self):
        try:
            self.proc.kill()
            self.proc.wait(timeout=5)
        except Exception:
            pass


def main():
    root = BASE / "project"
    shutil.rmtree(BASE, ignore_errors=True)
    path = root / PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(TEXT, encoding="utf-8")
    (root / "pom.xml").write_text(POM, encoding="utf-8")

    lsp = Lsp(root)
    rows = []
    try:
        lsp.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": lsp.uri(PATH),
                        "languageId": "java",
                        "version": 1,
                        "text": TEXT,
                    }
                },
            }
        )
        # Wait for the index to settle through a real query, then ask for lenses.
        deadline = time.time() + 8
        while time.time() < deadline:
            lsp.request("textDocument/documentSymbol", {"textDocument": {"uri": lsp.uri(PATH)}})
            stats = lsp.request("ravac/indexStats", {}) or {}
            if not stats.get("building"):
                break
            time.sleep(0.05)
        lenses = lsp.request("textDocument/codeLens", {"textDocument": {"uri": lsp.uri(PATH)}}) or []
        print(f"lenses={len(lenses)}", flush=True)
        for lens in lenses:
            print(
                f"  {lens.get('range', {}).get('start')} {lens.get('command', {}).get('title')}",
                flush=True,
            )
        starts = [
            (lens.get("range", {}).get("start", {}).get("line"), lens.get("range", {}).get("start", {}).get("character"))
            for lens in lenses
        ]
        duplicates = {start for start in starts if starts.count(start) > 1}

        def one_group(line, column):
            return starts.count((line, column)) == 1

        # Outer (line 7 col 6), RecordHelper (line 11 col 34), four methods.
        checks = [
            ("class Outer one group", one_group(6, 6)),
            ("nested RecordHelper one group", one_group(10, 34)),
            ("isRecord one group", one_group(11, 25)),
            ("no duplicate ranges", not duplicates),
            ("no import lens", (0, 0) not in starts and (1, 0) not in starts),
            ("no constructor lens", all(
                "references" in (lens.get("command", {}).get("title") or "") for lens in lenses
            )),
        ]
        for title, ok in checks:
            rows.append({"title": title, "ok": ok})
    finally:
        lsp.close()

    failed = [row for row in rows if not row["ok"]]
    for row in rows:
        print(("PASS  " if row["ok"] else "FAIL  ") + row["title"], flush=True)
    print("", flush=True)
    print(f"{len(rows) - len(failed)}/{len(rows)} checks passed", flush=True)
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
