#!/usr/bin/env python3
"""Reproduce the A-audit stall: multi-module fixture, didOpen, then wait for
publishDiagnostics with a hard timeout and record the timeline.

Usage:
    py scripts/lsp/probe_b_stall.py [--timeout 10]
"""

import json
import os
import queue
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
BASE = ROOT / "target" / "tmp" / "b-stall" / ("run-" + str(os.getpid()))

POM = (
    "<project><modelVersion>4.0.0</modelVersion><groupId>com.audit</groupId>"
    "<artifactId>parent</artifactId><version>1.0</version><packaging>pom</packaging>"
    "<modules><module>m-a</module><module>m-b</module></modules>"
    "<properties><maven.compiler.release>17</maven.compiler.release></properties></project>\n"
)
PARENT = (
    "<parent><groupId>com.audit</groupId><artifactId>parent</artifactId>"
    "<version>1.0</version><relativePath>../pom.xml</relativePath></parent>"
)


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def main():
    timeout = 10.0
    if "--timeout" in sys.argv:
        timeout = float(sys.argv[sys.argv.index("--timeout") + 1])
    shutil.rmtree(BASE, ignore_errors=True)
    project = BASE / "project"
    write(project / "pom.xml", POM)
    write(project / "m-a/pom.xml", f"<project><modelVersion>4.0.0</modelVersion>{PARENT}<artifactId>m-a</artifactId></project>\n")
    write(project / "m-b/pom.xml", f"<project><modelVersion>4.0.0</modelVersion>{PARENT}<artifactId>m-b</artifactId></project>\n")
    write(
        project / "m-a/src/main/java/com/audit/ma/A.java",
        "package com.audit.ma;\n\nimport com.audit.mb.BClass;\n\npublic class A {\n    void f() {\n        BClass b = new BClass();\n    }\n}\n",
    )
    write(
        project / "m-b/src/main/java/com/audit/mb/BClass.java",
        "package com.audit.mb;\n\npublic class BClass {\n}\n",
    )
    if "--prebuilt" in sys.argv:
        (project / "target" / "classes").mkdir(parents=True, exist_ok=True)

    proc = subprocess.Popen(
        [str(EXE)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    messages: "queue.Queue[dict]" = queue.Queue()

    def reader():
        while True:
            headers = b""
            while b"\r\n\r\n" not in headers:
                chunk = proc.stdout.read(1)
                if not chunk:
                    return
                headers += chunk
            try:
                length = int(headers.split(b"Content-Length: ")[1].split(b"\r\n")[0])
            except Exception:
                return
            body = proc.stdout.read(length)
            try:
                messages.put(json.loads(body))
            except Exception:
                return

    threading.Thread(target=reader, daemon=True).start()

    def send(message):
        body = json.dumps(message).encode()
        proc.stdin.write(b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)
        proc.stdin.flush()

    def wait(method_or_id=None, timeout=timeout):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                message = messages.get(timeout=deadline - time.time())
            except queue.Empty:
                return None
            if method_or_id is None:
                if message.get("method") == "textDocument/publishDiagnostics":
                    return message
            elif message.get("id") == method_or_id:
                return message
            elif message.get("method") == method_or_id:
                return message
        return None

    started = time.time()
    send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"rootUri": project.as_uri()}})
    init = wait(1, timeout)
    print(f"[{time.time() - started:7.3f}] initialize -> {'ok' if init else 'TIMEOUT'}", flush=True)
    send({"jsonrpc": "2.0", "id": 2, "method": "ravac/indexStats", "params": {}})
    stats = wait(2, timeout)
    print(f"[{time.time() - started:7.3f}] indexStats -> {json.dumps(stats)[:160] if stats else 'TIMEOUT'}", flush=True)

    # Wait until the private artifact build has finished, like the A-audit
    # probe did while polling indexStats for 20s.
    deadline = time.time() + 25
    last_stats = None
    while time.time() < deadline:
        send({"jsonrpc": "2.0", "id": 20, "method": "ravac/artifactStats", "params": {}})
        response = wait(20, 5)
        last_stats = (response or {}).get("result")
        print(f"[{time.time() - started:7.3f}] artifactStats -> {json.dumps(last_stats)[:160] if last_stats else 'none'}", flush=True)
        if last_stats and last_stats.get("status") != "building":
            break
        time.sleep(0.2)
    send({"jsonrpc": "2.0", "id": 2, "method": "ravac/indexStats", "params": {}})
    stats = wait(2, timeout)
    print(f"[{time.time() - started:7.3f}] indexStats-after-artifacts -> {json.dumps(stats)[:160] if stats else 'TIMEOUT'}", flush=True)

    app = project / "m-a/src/main/java/com/audit/ma/A.java"
    text = app.read_text(encoding="utf-8")
    send(
        {
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": app.as_uri(),
                    "languageId": "java",
                    "version": 1,
                    "text": text,
                }
            },
        }
    )
    diagnostics = wait(None, timeout)
    if diagnostics:
        print(
            f"[{time.time() - started:7.3f}] publishDiagnostics ok (n={len((diagnostics.get('params') or {}).get('diagnostics') or [])})",
            flush=True,
        )
    else:
        print(f"[{time.time() - started:7.3f}] publishDiagnostics TIMEOUT after {timeout}s", flush=True)
        send(
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": {"uri": app.as_uri()},
                    "position": {"line": 5, "character": 7},
                },
            }
        )
        hover = wait(3, 5)
        print(f"[{time.time() - started:7.3f}] hover after timeout -> {'ok' if hover else 'NO RESPONSE'}", flush=True)
        send({"jsonrpc": "2.0", "id": 4, "method": "textDocument/documentSymbol", "params": {"textDocument": {"uri": app.as_uri()}}})
        symbols = wait(4, 5)
        print(f"[{time.time() - started:7.3f}] documentSymbol after timeout -> {'ok' if symbols else 'NO RESPONSE'}", flush=True)

    try:
        send({"jsonrpc": "2.0", "id": 9, "method": "shutdown", "params": {}})
        wait(9, 2)
        send({"jsonrpc": "2.0", "method": "exit", "params": {}})
        proc.wait(timeout=5)
    except Exception:
        proc.kill()
    print(f"fixture: {project}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
