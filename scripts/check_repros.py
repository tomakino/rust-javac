#!/usr/bin/env python3
"""Compile every minimal repro with javac and rust-javac, compare class SHA-256.

Usage: python scripts/check_repros.py [--dir DIR]

This is the fast feedback loop (a few seconds): use it after every change and
only run the full Guava comparison when it is green.
"""
import argparse
import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path


def resolve_tool(name, env_name):
    suffix = ".exe" if os.name == "nt" else ""
    home = os.environ.get(env_name)
    if home:
        candidate = Path(home) / "bin" / (name + suffix)
        if candidate.is_file():
            return str(candidate)
    return shutil.which(name) or ""


def hashes(root):
    out = {}
    if not root.is_dir():
        return out
    for path in sorted(root.rglob("*.class")):
        out[path.relative_to(root).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return out


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", default=str(root / "target" / "tmp" / "minrepro3"))
    args = parser.parse_args()

    src_dir = Path(args.dir)
    rust = root / "target" / "debug" / ("ravac.exe" if os.name == "nt" else "ravac")
    javac = resolve_tool("javac", "JAVA_HOME")
    if not javac:
        print("javac not found", file=sys.stderr)
        return 2
    if not rust.is_file():
        print("rust-javac not found: {}".format(rust), file=sys.stderr)
        return 2

    repros = sorted(p for p in src_dir.glob("R*.java"))
    if not repros:
        print("no repros in {}".format(src_dir), file=sys.stderr)
        return 2

    passed = failed = 0
    failures = []
    for src in repros:
        stem = src.stem
        j_out = src_dir / ("oj_" + stem)
        r_out = src_dir / ("or_" + stem)
        for d in (j_out, r_out):
            shutil.rmtree(d, ignore_errors=True)
            d.mkdir(parents=True, exist_ok=True)
        pj = subprocess.run([javac, "-d", str(j_out), str(src)], capture_output=True, text=True)
        pr = subprocess.run([str(rust), "-d", str(r_out), str(src)], capture_output=True, text=True)
        jh, rh = hashes(j_out), hashes(r_out)
        ok = pj.returncode == 0 and pr.returncode == 0 and jh == rh and jh
        if ok:
            passed += 1
            print("PASS " + src.name)
        else:
            failed += 1
            detail = []
            if pj.returncode != 0:
                detail.append("javac exit {}".format(pj.returncode))
            if pr.returncode != 0:
                detail.append("rust exit {}".format(pr.returncode))
            missing = sorted(set(jh) - set(rh))
            extra = sorted(set(rh) - set(jh))
            diff = sorted(n for n in set(jh) & set(rh) if jh[n] != rh[n])
            if missing:
                detail.append("missing {}".format(len(missing)))
            if extra:
                detail.append("extra {}".format(len(extra)))
            if diff:
                detail.append("differs {}".format(len(diff)))
            print("FAIL {} ({})".format(src.name, ", ".join(detail) or "sha mismatch"))
            failures.append(src.name)

    print("repros: {}/{} passed".format(passed, passed + failed))
    if failures:
        print("failed: " + ", ".join(failures))
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
