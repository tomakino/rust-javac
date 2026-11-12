#!/usr/bin/env python3
"""RxJava parity check: compile the RxJava 3 main sources with javac and
rust-javac, then compare the produced class files by count and SHA-256.

RxJava's only compile-time dependency is ``org.reactivestreams``; its sources live
under ``rxjava-deps`` and are compiled together with the project.

Usage:
    python scripts/check_rxjava.py [--build] [--verbose] [--release N]

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


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="cargo build -p rust-javac first")
    parser.add_argument("--verbose", action="store_true", help="print the first differing classes")
    parser.add_argument("--release", help="pass --release N to both compilers")
    args = parser.parse_args()

    projects = root / "internal" / "test" / "real-projects"
    rx_root = projects / "rxjava-3.1.8-src"
    deps_root = projects / "rxjava-deps"
    suffix = ".exe" if os.name == "nt" else ""
    rust = root / "target" / "debug" / ("ravac" + suffix)
    javac = resolve_tool("javac", "JAVA_HOME")

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not rx_root.is_dir():
        print("rxjava sources not found: {}".format(rx_root), file=sys.stderr)
        return 2
    if not rust.is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2
    if not javac:
        print("javac not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2

    t_start = time.perf_counter()
    sources = sorted(str(p) for p in rx_root.rglob("*.java"))
    if deps_root.is_dir():
        sources += sorted(str(p) for p in deps_root.rglob("*.java"))
    if not sources:
        print("no java sources below {}".format(rx_root), file=sys.stderr)
        return 2
    print("rxjava sources={}".format(len(sources)))

    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    temp = Path(tempfile.mkdtemp(prefix="rxjava-", dir=str(tmp_root)))
    try:
        j_out = temp / "javac"
        r_out = temp / "rust"
        j_out.mkdir()
        r_out.mkdir()
        common = ["-encoding", "UTF-8", "-J-Duser.language=en", "-J-Duser.country=US"]
        if args.release:
            common = common + ["--release", args.release]
        for out_dir in (j_out, r_out):
            lines = ["-d", str(out_dir)] + sources
            (temp / (out_dir.name + ".args")).write_text("\n".join(lines), encoding="utf-8")

        t_parallel = time.perf_counter()
        p_javac = subprocess.Popen(
            [javac] + common + ["@" + str(temp / "javac.args")],
            cwd=str(temp),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        t_rust = time.perf_counter()
        p_rust = subprocess.Popen(
            [str(rust)] + common + ["@" + str(temp / "rust.args")],
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
        print("timing: javac={:.2f}s rust={:.2f}s".format(t_parallel, t_rust))
        if args.verbose:
            for line in (j_err or "").splitlines()[:3]:
                print("javac stderr: " + line)
            for line in (r_err or "").splitlines()[:3]:
                print("rust stderr: " + line)

        j_classes = hash_dir(j_out, ".class")
        r_classes = hash_dir(r_out, ".class")
        print("javac classes={} rust classes={}".format(len(j_classes), len(r_classes)))

        missing = sorted(set(j_classes) - set(r_classes))
        extra = sorted(set(r_classes) - set(j_classes))
        differing = sorted(
            name for name in set(j_classes) & set(r_classes) if j_classes[name] != r_classes[name]
        )
        if missing:
            print("missing in rust: {}".format(len(missing)))
            for name in missing[:5]:
                print("  MISSING " + name)
        if extra:
            print("extra in rust: {}".format(len(extra)))
            for name in extra[:5]:
                print("  EXTRA " + name)
        if differing:
            print("sha256 differs: {}".format(len(differing)))
            for name in differing[:5]:
                print("  DIFF " + name)
                if args.verbose:
                    print("    javac=" + j_classes[name])
                    print("    rust =" + r_classes[name])

        ok = (
            j_code == 0
            and r_code == 0
            and not missing
            and not extra
            and not differing
            and len(j_classes) == len(r_classes)
        )
        print("total={:.2f}s".format(time.perf_counter() - t_start))
        print("result: {}".format("PASS" if ok else "FAIL"))
        return 0 if ok else 1
    finally:
        shutil.rmtree(temp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
