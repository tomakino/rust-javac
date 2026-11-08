#!/usr/bin/env python3
"""F: performance and scale audit through a real LSP.

Measures cold index build (82 / 1000 files, batches of 128), buffer-less
queries before the index is ready, large-file analysis (5000 / 10000 lines),
concurrent requests, $/cancelRequest, RSS + dump size and 20 mixed rounds.

Usage:
    py scripts/lsp/probe_f_performance.py [--exe PATH] [--scale 82,1000]
"""

import csv
import io
import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "debug" / "ravac-lsp.exe"
BASE = ROOT / "target" / "tmp" / "f-performance" / ("run-" + str(os.getpid()))
INDEX_DIR = Path(os.environ.get("LOCALAPPDATA", "")) / "ravac" / "index"
TEMP_INDEX = Path(tempfile.gettempdir()) / "ravac" / "index"

POM = (
    "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId>"
    "<artifactId>perf</artifactId><version>1.0</version></project>\n"
)


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))


def gen_project(root: Path, count: int, chained: bool) -> list[str]:
    """Write `count` classes. Chained projects reference the previous class (a
    realistic cross-file closure, one batch at 82 files); the 1000-file
    project uses independent classes so batches compile on their own."""
    files = []
    for index in range(count):
        package = f"p{index // 100}"
        class_name = f"C{index}"
        body = [
            f"package com.example.{package};",
            "",
        ]
        if chained and index > 0:
            previous = f"C{index - 1}"
            previous_package = f"p{(index - 1) // 100}"
            if previous_package != package:
                body.append(f"import com.example.{previous_package}.{previous};")
                body.append("")
        body.append(f"public class {class_name} {{")
        body.append(f"    int value{index};")
        body.append(f"    String name{index};")
        body.append("")
        body.append(f"    int compute{index}(int a) {{")
        body.append(f"        return a + value{index};")
        body.append("    }")
        body.append("")
        body.append(f"    int use{index}() {{")
        if chained and index > 0:
            previous = f"C{index - 1}"
            body.append(f"        {previous} other = new {previous}();")
            body.append(f"        return other.compute{index - 1}(value{index});")
        else:
            body.append(f"        return value{index};")
        body.append("    }")
        body.append("}")
        body.append("")
        rel = f"src/main/java/com/example/{package}/{class_name}.java"
        write(root / rel, "\n".join(body))
        files.append(rel)
    return files


def tasklist_rss_kb(pid: int) -> int:
    try:
        out = subprocess.run(
            ["tasklist", "/FI", f"PID eq {pid}", "/FO", "CSV", "/NH"],
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout
        rows = list(csv.reader(io.StringIO(out)))
        if not rows:
            return 0
        value = rows[0][-1].strip()
        # "12,345 K"
        value = value.replace(",", "").replace("K", "").strip()
        return int(value)
    except Exception:
        return 0


def latest_dump() -> tuple[str, int]:
    best = None
    for base in (INDEX_DIR, TEMP_INDEX):
        if not base.is_dir():
            continue
        for entry in base.glob("*.bin"):
            if best is None or entry.stat().st_mtime > best.stat().st_mtime:
                best = entry
    if best is None:
        return "", 0
    return str(best), best.stat().st_size


class Lsp:
    def __init__(self, root: Path, exe: Path = EXE):
        self.root = root
        self.exe = exe
        self.proc = subprocess.Popen(
            [str(exe)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        self.messages: "queue.Queue[dict]" = queue.Queue()
        self.batches: list[dict] = []
        self.stderr_lines: list[str] = []
        threading.Thread(target=self._reader, daemon=True).start()
        threading.Thread(target=self._stderr_reader, daemon=True).start()
        self.next_id = 100

    def _reader(self):
        while True:
            headers = b""
            while b"\r\n\r\n" not in headers:
                chunk = self.proc.stdout.read(1)
                if not chunk:
                    return
                headers += chunk
            length = int(headers.split(b"Content-Length: ")[1].split(b"\r\n")[0])
            self.messages.put(json.loads(self.proc.stdout.read(length)))

    def _stderr_reader(self):
        while True:
            line = self.proc.stderr.readline()
            if not line:
                return
            text = line.decode("utf-8", "replace").strip()
            self.stderr_lines.append(text)
            if text.startswith("[index-batch]"):
                fields = dict(
                    part.split("=", 1) for part in text.split()[1:] if "=" in part
                )
                self.batches.append(
                    {
                        "files": int(fields.get("files", 0)),
                        "indexed": int(fields.get("indexed", 0)),
                        "ms": int(fields.get("ms", 0)),
                    }
                )

    def send(self, message):
        body = json.dumps(message).encode()
        self.proc.stdin.write(
            b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body
        )
        self.proc.stdin.flush()

    def read_until(self, request_id, timeout: float = 60.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                message = self.messages.get(timeout=max(0.0, deadline - time.time()))
            except queue.Empty:
                return None
            if message.get("id") == request_id:
                return message
        return None

    def request(self, method, params, request_id=None, timeout: float = 60.0):
        if request_id is None:
            self.next_id += 1
            request_id = self.next_id
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        return (self.read_until(request_id, timeout=timeout) or {}).get("result")

    def uri(self, relative: str) -> str:
        return (self.root / relative).as_uri()

    def did_open(self, relative: str, text: str):
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

    def close(self):
        try:
            self.proc.kill()
            self.proc.wait(timeout=5)
        except Exception:
            pass


def timed(fn):
    started = time.perf_counter()
    value = fn()
    return value, (time.perf_counter() - started) * 1000.0


def cold_index(name: str, count: int, exe: Path, chained: bool) -> dict:
    root = BASE / name / "project"
    shutil.rmtree(root.parent, ignore_errors=True)
    write(root / "pom.xml", POM)
    files = gen_project(root, count, chained)
    lsp = Lsp(root, exe)
    result = {}
    try:
        rss_before = tasklist_rss_kb(lsp.proc.pid)
        started = time.perf_counter()
        initialize = lsp.request("initialize", {"rootUri": root.as_uri()}, request_id=1)
        result["initialize_ms"] = (time.perf_counter() - started) * 1000.0
        print(f"  [cold] initialize={result['initialize_ms']:.0f}ms", flush=True)
        # Trigger the lazy index through a real query, then poll its status.
        target = files[0]
        position = {"line": 3, "character": 4}
        _, first_query_ms = timed(
            lambda: lsp.request(
                "textDocument/definition",
                {"textDocument": {"uri": lsp.uri(target)}, "position": position},
            )
        )
        result["first_query_ms"] = first_query_ms
        print(f"  [cold] first_query={first_query_ms:.0f}ms", flush=True)
        deadline = time.time() + 300
        generation = 0
        while time.time() < deadline:
            stats = lsp.request("ravac/indexStats", {}) or {}
            generation = int(stats.get("generation") or 0)
            if not stats.get("building") and generation > 0:
                break
            if int((time.perf_counter() - started) * 1000) % 2000 < 25:
                print(f"  [cold] poll elapsed={(time.perf_counter() - started):.1f}s building={stats.get('building')} gen={generation}", flush=True)
            time.sleep(0.02)
        result["index_ready_ms"] = (time.perf_counter() - started) * 1000.0
        result["generation"] = generation
        result["batches"] = list(lsp.batches)
        result["stderr_bytes"] = sum(len(line) for line in lsp.stderr_lines)
        result["panic"] = any("panic" in line.lower() for line in lsp.stderr_lines)
        result["rss_before_kb"] = rss_before
        result["rss_ready_kb"] = tasklist_rss_kb(lsp.proc.pid)
        dump_path, dump_size = latest_dump()
        result["dump_path"] = dump_path
        result["dump_bytes"] = dump_size

        # 20 mixed rounds on five files (hover/definition/documentSymbol/
        # semanticTokens/selectionRange), measuring drift and RSS.
        picks = [files[index % len(files)] for index in range(5)]
        rounds = []
        for round_index in range(20):
            rel = picks[round_index % len(picks)]
            text = (root / rel).read_text(encoding="utf-8")
            line = next(
                (idx for idx, row in enumerate(text.split("\n")) if "void" not in row and "compute" in row),
                0,
            )
            started_round = time.perf_counter()
            lsp.request(
                "textDocument/hover",
                {"textDocument": {"uri": lsp.uri(rel)}, "position": {"line": 4, "character": 8}},
            )
            lsp.request(
                "textDocument/definition",
                {"textDocument": {"uri": lsp.uri(rel)}, "position": {"line": 10, "character": 15}},
            )
            lsp.request("textDocument/documentSymbol", {"textDocument": {"uri": lsp.uri(rel)}})
            lsp.request("textDocument/semanticTokens/full", {"textDocument": {"uri": lsp.uri(rel)}})
            lsp.request(
                "textDocument/selectionRange",
                {
                    "textDocument": {"uri": lsp.uri(rel)},
                    "positions": [{"line": 4, "character": 8}],
                },
            )
            rounds.append((time.perf_counter() - started_round) * 1000.0)
        result["rounds_ms"] = rounds
        result["rss_after_rounds_kb"] = tasklist_rss_kb(lsp.proc.pid)
        result["panic_after_rounds"] = any(
            "panic" in line.lower() for line in lsp.stderr_lines
        )
    finally:
        lsp.close()
    return result


def early_requests(name: str, count: int, exe: Path) -> dict:
    """Query immediately after initialize, before the index is ready."""
    root = BASE / name / "project"
    shutil.rmtree(root.parent, ignore_errors=True)
    write(root / "pom.xml", POM)
    files = gen_project(root, count, True)
    lsp = Lsp(root, exe)
    result = {"queries": []}
    try:
        started = time.perf_counter()
        lsp.request("initialize", {"rootUri": root.as_uri()}, request_id=1)
        result["initialize_ms"] = (time.perf_counter() - started) * 1000.0
        print(f"  [cold] initialize={result['initialize_ms']:.0f}ms", flush=True)
        for relative in files[:3]:
            for kind, call in [
                (
                    "definition",
                    lambda rel=relative: lsp.request(
                        "textDocument/definition",
                        {
                            "textDocument": {"uri": lsp.uri(rel)},
                            "position": {"line": 3, "character": 4},
                        },
                    ),
                ),
                (
                    "hover",
                    lambda rel=relative: lsp.request(
                        "textDocument/hover",
                        {
                            "textDocument": {"uri": lsp.uri(rel)},
                            "position": {"line": 3, "character": 4},
                        },
                    ),
                ),
                (
                    "documentSymbol",
                    lambda rel=relative: lsp.request(
                        "textDocument/documentSymbol",
                        {"textDocument": {"uri": lsp.uri(rel)}},
                    ),
                ),
            ]:
                value, elapsed = timed(call)
                result["queries"].append(
                    {
                        "file": relative,
                        "kind": kind,
                        "ms": elapsed,
                        "empty": value is None or value == [],
                    }
                )
        # Finish the index to report its total time as well.
        deadline = time.time() + 300
        while time.time() < deadline:
            stats = lsp.request("ravac/indexStats", {}) or {}
            if not stats.get("building") and int(stats.get("generation") or 0) > 0:
                break
            time.sleep(0.02)
        result["index_ready_ms"] = (time.perf_counter() - started) * 1000.0
        result["batches"] = list(lsp.batches)
        result["panic"] = any("panic" in line.lower() for line in lsp.stderr_lines)
    finally:
        lsp.close()
    return result


def large_file(name: str, line_count: int, exe: Path) -> dict:
    root = BASE / name / "project"
    shutil.rmtree(root.parent, ignore_errors=True)
    write(root / "pom.xml", POM)
    body = "".join(f"        x = x + {index};\n" for index in range(line_count))
    text = (
        "package com.example;\n\n"
        "public class Big {\n"
        "    int value;\n\n"
        "    int get() {\n"
        "        return value;\n"
        "    }\n\n"
        "    void f() {\n"
        "        int x = value;\n"
        + body
        + "    }\n}\n"
    )
    rel = "src/main/java/com/example/Big.java"
    write(root / rel, text)
    lsp = Lsp(root, exe)
    result = {"lines": text.count("\n") + 1}
    try:
        lsp.request("initialize", {"rootUri": root.as_uri()}, request_id=1)
        started = time.perf_counter()
        lsp.did_open(rel, text)
        # The didOpen handler compiles synchronously; wait for its diagnostics.
        deadline = time.time() + 120
        while time.time() < deadline:
            try:
                message = lsp.messages.get(timeout=0.5)
            except queue.Empty:
                continue
            if message.get("method") == "textDocument/publishDiagnostics":
                result["did_open_ms"] = (time.perf_counter() - started) * 1000.0
                break
        position = {"line": 9, "character": 16}
        first_hover, first_hover_ms = timed(
            lambda: lsp.request(
                "textDocument/hover",
                {"textDocument": {"uri": lsp.uri(rel)}, "position": position},
            )
        )
        first_def, first_def_ms = timed(
            lambda: lsp.request(
                "textDocument/definition",
                {"textDocument": {"uri": lsp.uri(rel)}, "position": position},
            )
        )
        second_hover, second_hover_ms = timed(
            lambda: lsp.request(
                "textDocument/hover",
                {"textDocument": {"uri": lsp.uri(rel)}, "position": position},
            )
        )
        second_def, second_def_ms = timed(
            lambda: lsp.request(
                "textDocument/definition",
                {"textDocument": {"uri": lsp.uri(rel)}, "position": position},
            )
        )
        result.update(
            {
                "first_hover_ms": first_hover_ms,
                "first_definition_ms": first_def_ms,
                "second_hover_ms": second_hover_ms,
                "second_definition_ms": second_def_ms,
                "hover_ok": first_hover is not None,
                "definition_ok": first_def is not None,
                "rss_kb": tasklist_rss_kb(lsp.proc.pid),
                "panic": any("panic" in line.lower() for line in lsp.stderr_lines),
            }
        )
    finally:
        lsp.close()
    return result


def concurrent(name: str, count: int, exe: Path) -> dict:
    root = BASE / name / "project"
    shutil.rmtree(root.parent, ignore_errors=True)
    write(root / "pom.xml", POM)
    files = gen_project(root, count, False)
    lsp = Lsp(root, exe)
    result = {}
    try:
        lsp.request("initialize", {"rootUri": root.as_uri()}, request_id=1)
        # Warm the index and one analysis before measuring concurrency.
        lsp.request(
            "textDocument/definition",
            {
                "textDocument": {"uri": lsp.uri(files[0])},
                "position": {"line": 3, "character": 4},
            },
        )
        deadline = time.time() + 120
        while time.time() < deadline:
            stats = lsp.request("ravac/indexStats", {}) or {}
            if not stats.get("building") and int(stats.get("generation") or 0) > 0:
                break
            time.sleep(0.02)
        lsp.did_open(files[0], (root / files[0]).read_text(encoding="utf-8"))

        requests = []
        for index in range(10):
            rel = files[index % len(files)]
            requests.append(
                (
                    "textDocument/hover",
                    {"textDocument": {"uri": lsp.uri(rel)}, "position": {"line": 3, "character": 4}},
                )
            )
        for index in range(10):
            rel = files[index % len(files)]
            requests.append(
                (
                    "textDocument/definition",
                    {"textDocument": {"uri": lsp.uri(rel)}, "position": {"line": 3, "character": 4}},
                )
            )
        ids = []
        sent_at = {}
        started = time.perf_counter()
        for method, params in requests:
            lsp.next_id += 1
            request_id = lsp.next_id
            ids.append(request_id)
            sent_at[request_id] = time.perf_counter()
            lsp.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        arrivals = []
        remaining = set(ids)
        deadline = time.time() + 120
        while remaining and time.time() < deadline:
            try:
                message = lsp.messages.get(timeout=max(0.0, deadline - time.time()))
            except queue.Empty:
                break
            request_id = message.get("id")
            if request_id in remaining:
                remaining.discard(request_id)
                arrivals.append(
                    {
                        "id": request_id,
                        "at_ms": (time.perf_counter() - started) * 1000.0,
                        "latency_ms": (time.perf_counter() - sent_at[request_id]) * 1000.0,
                    }
                )
        result["total_ms"] = (time.perf_counter() - started) * 1000.0
        result["answered"] = len(arrivals)
        result["missing"] = sorted(remaining)
        result["latencies_ms"] = [entry["latency_ms"] for entry in arrivals]

        # $/cancelRequest after a request was already answered must not hurt.
        lsp.send({"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": ids[-1]}})
        alive, alive_ms = timed(
            lambda: lsp.request(
                "textDocument/hover",
                {"textDocument": {"uri": lsp.uri(files[0])}, "position": {"line": 3, "character": 4}},
            )
        )
        result["after_cancel_ok"] = alive is not None
        result["after_cancel_ms"] = alive_ms
        result["panic"] = any("panic" in line.lower() for line in lsp.stderr_lines)
    finally:
        lsp.close()
    return result


def main():
    exe = EXE
    if "--exe" in sys.argv:
        exe = Path(sys.argv[sys.argv.index("--exe") + 1]).resolve()
    scales = [82, 1000]
    if "--scale" in sys.argv:
        scales = [int(part) for part in sys.argv[sys.argv.index("--scale") + 1].split(",")]
    print(f"exe={exe}", flush=True)
    data = {"exe": str(exe), "scales": {}}
    for count in scales:
        name = f"cold-{count}"
        print(f"COLD {count} ...", flush=True)
        cold = cold_index(name, count, exe, chained=(count == 82))
        data["scales"][count] = cold
        print(
            f"  initialize={cold['initialize_ms']:.0f}ms first_query={cold['first_query_ms']:.0f}ms "
            f"index_ready={cold['index_ready_ms']:.0f}ms batches={len(cold['batches'])} "
            f"dump={cold['dump_bytes'] / 1024:.0f}KB rss={cold['rss_ready_kb'] / 1024:.0f}MB "
            f"rounds_avg={sum(cold['rounds_ms']) / len(cold['rounds_ms']):.0f}ms",
            flush=True,
        )
        print(f"  batches={cold['batches']}", flush=True)

    print("EARLY (before index ready) ...", flush=True)
    early = early_requests("early", 82, exe)
    data["early"] = early
    for query in early["queries"]:
        print(
            f"  {query['kind']} {query['file'].split('/')[-1]} {query['ms']:.0f}ms empty={query['empty']}",
            flush=True,
        )
    print(f"  index_ready_after_queries={early['index_ready_ms']:.0f}ms", flush=True)

    for line_count in (5000, 10000):
        print(f"LARGE {line_count} ...", flush=True)
        large = large_file(f"large-{line_count}", line_count, exe)
        data[f"large_{line_count}"] = large
        print(
            f"  lines={large['lines']} didOpen={large.get('did_open_ms', 0):.0f}ms "
            f"hover1={large['first_hover_ms']:.0f}ms def1={large['first_definition_ms']:.0f}ms "
            f"hover2={large['second_hover_ms']:.0f}ms def2={large['second_definition_ms']:.0f}ms "
            f"rss={large['rss_kb'] / 1024:.0f}MB",
            flush=True,
        )

    print("CONCURRENT 10 hover + 10 definition ...", flush=True)
    concurrency = concurrent("concurrent", 82, exe)
    data["concurrent"] = concurrency
    latencies = concurrency["latencies_ms"]
    print(
        f"  total={concurrency['total_ms']:.0f}ms answered={concurrency['answered']} "
        f"missing={len(concurrency['missing'])} "
        f"latency[min/med/max]={min(latencies) if latencies else 0:.0f}/"
        f"{sorted(latencies)[len(latencies) // 2] if latencies else 0:.0f}/"
        f"{max(latencies) if latencies else 0:.0f}ms "
        f"after_cancel={concurrency['after_cancel_ms']:.0f}ms",
        flush=True,
    )

    out = BASE / "data.json"
    out.write_text(json.dumps(data, indent=2), encoding="utf-8")
    print(f"data={out}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
