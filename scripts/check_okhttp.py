#!/usr/bin/env python3
"""OkHttp parity check: compile the OkHttp sources with javac and rust-javac,
then compare the produced class files by count and SHA-256.

Usage:
    python scripts/check_okhttp.py [--build] [--main-only] [--verbose]

Exit codes: 0 identical, 1 differences, 2 setup missing.
"""

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

MODULES = [
    "okhttp/src/main/java",
    "okhttp-generated",
    "okhttp-testing-support/src/main/java",
    "okhttp-tls/src/main/java",
    "mockwebserver/src/main/java",
    "okhttp-urlconnection/src/main/java",
    "okhttp-logging-interceptor/src/main/java",
]
TEST_MODULES = [
    "okhttp-tests/src/test/java",
    "mockwebserver/src/test/java",
    "okhttp-tls/src/test/java",
    "okhttp-urlconnection/src/test/java",
]


def resolve_tool(name: str, env_name: str) -> str:
    suffix = ".exe" if os.name == "nt" else ""
    home = os.environ.get(env_name)
    if home:
        candidate = Path(home) / "bin" / (name + suffix)
        if candidate.is_file():
            return str(candidate)
    return shutil.which(name) or ""


def hash_dir(root: Path, suffix: str) -> dict:
    result = {}
    if not root.is_dir():
        return result
    for path in sorted(root.rglob("*" + suffix)):
        if path.is_file():
            rel = path.relative_to(root).as_posix()
            result[rel] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def run(exe: str, args: list, cwd: Path):
    proc = subprocess.run(
        [exe] + args,
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return (
        proc.stdout.decode("utf-8", errors="replace"),
        proc.stderr.decode("utf-8", errors="replace"),
        proc.returncode,
    )


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="cargo build -p rust-javac first")
    parser.add_argument("--main-only", action="store_true", help="skip the test source sets")
    parser.add_argument("--verbose", action="store_true", help="print the first differing classes")
    args = parser.parse_args()

    projects = root / "internal" / "test" / "real-projects"
    ok_root = projects / "okhttp-parent-3.14.9"
    deps = projects / "okhttp-deps"
    suffix = ".exe" if os.name == "nt" else ""
    rust = root / "target" / "debug" / ("ravac" + suffix)
    javac = resolve_tool("javac", "JAVA_HOME")

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not ok_root.is_dir():
        print("okhttp sources not found: {}".format(ok_root), file=sys.stderr)
        return 2
    if not rust.is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2
    if not javac:
        print("javac not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2

    t_start = time.perf_counter()
    t_scan = time.perf_counter()
    modules = list(MODULES) if args.main_only else MODULES + TEST_MODULES
    sources = []
    for module in modules:
        directory = ok_root / module
        if directory.is_dir():
            sources += sorted(str(p) for p in directory.rglob("*.java") if p.name != "module-info.java")
    jars = sorted(str(p) for p in deps.glob("*.jar")) if deps.is_dir() else []
    classpath = os.pathsep.join(jars)
    t_scan = time.perf_counter() - t_scan
    print("okhttp modules={} sources={} jars={}".format(len(modules), len(sources), len(jars)))

    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    temp = Path(tempfile.mkdtemp(prefix="okhttp-", dir=str(tmp_root)))
    try:
        j_out = temp / "javac"
        r_out = temp / "rust"
        j_out.mkdir()
        r_out.mkdir()
        before = ["-J-Duser.language=en", "-J-Duser.country=US"]
        for out_dir in (j_out, r_out):
            lines = []
            if classpath:
                lines += ["-cp", classpath]
            lines += ["-d", str(out_dir)] + sources
            (temp / (out_dir.name + ".args")).write_text("\n".join(lines), encoding="utf-8")
        # The two compilers are independent processes writing to separate output
        # directories, so start them together: the wall clock is the slower of
        # the two instead of their sum.
        t_parallel = time.perf_counter()
        p_javac = subprocess.Popen(
            [javac] + before + ["@" + str(temp / "javac.args")],
            cwd=str(temp),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        t_rust = time.perf_counter()
        p_rust = subprocess.Popen(
            [str(rust)] + before + ["@" + str(temp / "rust.args")],
            cwd=str(temp),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        _, r_err_bytes = p_rust.communicate()
        t_rust = time.perf_counter() - t_rust
        _, j_err_bytes = p_javac.communicate()
        t_parallel = time.perf_counter() - t_parallel

        j_err = j_err_bytes.decode("utf-8", "replace")
        r_err = r_err_bytes.decode("utf-8", "replace")
        j_code = p_javac.returncode
        r_code = p_rust.returncode
        print("javac exit={} rust exit={}".format(j_code, r_code))
        if j_err:
            print("javac stderr (tail):")
            for line in j_err.splitlines()[-5:]:
                print("  " + line)
        if r_err:
            # Keep only lines that carry a diagnostic; the "^^^" caret lines are
            # source excerpts and say nothing on their own.
            errors = [
                line
                for line in r_err.splitlines()
                if "error:" in line or "错误:" in line
            ]
            if errors:
                print("rust errors:")
                for line in errors[:10]:
                    print("  " + line)
            print("rust stderr (tail):")
            for line in r_err.splitlines()[-4:]:
                print("  " + line)

        t_hash = time.perf_counter()
        j_classes = hash_dir(j_out, ".class")
        r_classes = hash_dir(r_out, ".class")
        t_hash = time.perf_counter() - t_hash
        print("javac classes={} rust classes={}".format(len(j_classes), len(r_classes)))

        missing = sorted(set(j_classes) - set(r_classes))
        extra = sorted(set(r_classes) - set(j_classes))
        differing = sorted(
            name for name in set(j_classes) & set(r_classes) if j_classes[name] != r_classes[name]
        )
        for label, items in (("missing in rust", missing), ("extra in rust", extra), ("sha256 differs", differing)):
            if items:
                print("{}: {}".format(label, len(items)))
                for name in items[:5]:
                    print("  " + name)
        if args.verbose and differing:
            name = differing[0]
            print("  javac=" + j_classes[name])
            print("  rust =" + r_classes[name])

        ok = (
            j_code == 0
            and r_code == 0
            and not missing
            and not extra
            and not differing
            and len(j_classes) == len(r_classes)
        )
        t_total = time.perf_counter() - t_start
        t_setup = t_total - t_scan - t_parallel - t_hash
        print(
            "breakdown: scan={:.2f}s compile={:.2f}s (rust={:.2f}s) hash={:.2f}s "
            "other={:.2f}s total={:.2f}s".format(
                t_scan, t_parallel, t_rust, t_hash, t_setup, t_total
            )
        )
        print("result: {}".format("PASS" if ok else "FAIL"))
        return 0 if ok else 1
    finally:
        shutil.rmtree(temp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
