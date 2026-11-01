#!/usr/bin/env python3
"""Gson parity check: compile the Gson main sources with javac and rust-javac,
then compare the produced class files by count and SHA-256.

Usage:
    python scripts/check_gson.py [--build] [--verbose]

Exit codes: 0 identical, 1 differences, 2 setup missing.
"""

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def resolve_tool(name: str, env_name: str, fallback: str = None) -> str:
    suffix = ".exe" if os.name == "nt" else ""
    home = os.environ.get(env_name)
    if home:
        candidate = Path(home) / "bin" / (name + suffix)
        if candidate.is_file():
            return str(candidate)
    found = shutil.which(name)
    if found:
        return found
    if fallback and Path(fallback).is_file():
        return fallback
    return ""


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
    parser.add_argument("--verbose", action="store_true", help="print the first differing classes")
    args = parser.parse_args()

    projects = root / "internal" / "test" / "real-projects"
    gson_root = projects / "gson-gson-parent-2.10.1" / "gson" / "src" / "main" / "java"
    generated = projects / "gson-crosscheck" / "generated"
    suffix = ".exe" if os.name == "nt" else ""
    rust = root / "target" / "debug" / ("rust-javac" + suffix)
    javac = resolve_tool("javac", "JAVA_HOME")

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not gson_root.is_dir():
        print("gson sources not found: {}".format(gson_root), file=sys.stderr)
        return 2
    if not rust.is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2
    if not javac:
        print("javac not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2

    sources = sorted(str(p) for p in gson_root.rglob("*.java"))
    if generated.is_dir():
        sources += sorted(str(p) for p in generated.rglob("*.java"))
    print("gson sources={}".format(len(sources)))

    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    temp = Path(tempfile.mkdtemp(prefix="gson-", dir=str(tmp_root)))
    try:
        j_out = temp / "javac"
        r_out = temp / "rust"
        j_out.mkdir()
        r_out.mkdir()
        common = ["-J-Duser.language=en", "-J-Duser.country=US"]
        for out_dir in (j_out, r_out):
            lines = ["-d", str(out_dir)] + sources
            (temp / (out_dir.name + ".args")).write_text("\n".join(lines), encoding="utf-8")
        _, j_err, j_code = run(javac, common + ["@" + str(temp / "javac.args")], temp)
        _, r_err, r_code = run(str(rust), common + ["@" + str(temp / "rust.args")], temp)
        print("javac exit={} rust exit={}".format(j_code, r_code))
        if args.verbose:
            if j_err:
                print("javac stderr (head): " + j_err.splitlines()[0])
            if r_err:
                print("rust stderr (head): " + r_err.splitlines()[0])

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
            j_code == r_code
            and not missing
            and not extra
            and not differing
            and len(j_classes) == len(r_classes)
        )
        print("result: {}".format("PASS" if ok else "FAIL"))
        return 0 if ok else 1
    finally:
        shutil.rmtree(temp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
