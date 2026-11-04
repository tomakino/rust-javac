#!/usr/bin/env python3
"""Minimal compile benchmark: rust-javac on the Gson corpus only.

No javac, no class-file hashing, no parity comparison -- just "how long does
the compiler take end to end". This exists because the parity checks report
only the two subprocess calls, which hides hashing and setup time.

Usage:
    python scripts/bench_rust_javac.py [--repeat N] [--build] [--corpus gson|okhttp]

Exit codes: 0 ok, 2 setup missing.
"""

import argparse
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
    found = shutil.which(name)
    return found or ""


CORPORA = {
    "gson": (
        "gson-gson-parent-2.10.1", "gson/src/main/java",
        "gson-crosscheck", "generated",
    ),
    "okhttp": (
        "okhttp-parent-3.14.9", "okhttp/src/main/java",
        None, None,
    ),
}


def collect_sources(projects: Path, name: str) -> list:
    root_dir, main_sub, gen_dir, gen_sub = CORPORA[name]
    main_root = projects / root_dir / main_sub
    if not main_root.is_dir():
        return []
    sources = sorted(str(p) for p in main_root.rglob("*.java"))
    if gen_dir:
        gen_root = projects / gen_dir / gen_sub
        if gen_root.is_dir():
            sources += sorted(str(p) for p in gen_root.rglob("*.java"))
    return sources


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeat", type=int, default=5, help="timed runs (default 5)")
    parser.add_argument("--build", action="store_true", help="cargo build -p rust-javac first")
    parser.add_argument("--corpus", default="gson", choices=sorted(CORPORA))
    parser.add_argument("--warmup", type=int, default=1, help="untimed warmup runs (default 1)")
    args = parser.parse_args()

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2

    rust = resolve_tool("rust-javac", "JAVAC_HOME_RUST")
    if not rust:
        rust = str(root / "target" / "debug" / ("ravac.exe" if os.name == "nt" else "ravac"))
    if not Path(rust).is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2

    projects = root / "internal" / "test" / "real-projects"
    sources = collect_sources(projects, args.corpus)
    if not sources:
        print("{} sources not found".format(args.corpus), file=sys.stderr)
        return 2
    print("corpus={} sources={}".format(args.corpus, len(sources)))

    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    temp = Path(tempfile.mkdtemp(prefix="bench-", dir=str(tmp_root)))
    timings = []
    try:
        for i in range(args.warmup + args.repeat):
            out_dir = temp / ("out{}".format(i))
            out_dir.mkdir()
            arg_file = temp / ("run{}.args".format(i))
            arg_file.write_text("\n".join(["-d", str(out_dir)] + sources), encoding="utf-8")
            t0 = time.perf_counter()
            proc = subprocess.run(
                [rust, "@" + str(arg_file)],
                cwd=str(temp),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            elapsed = time.perf_counter() - t0
            if proc.returncode != 0:
                print("run {} failed (exit {})".format(i, proc.returncode), file=sys.stderr)
                print(proc.stderr.decode("utf-8", "replace")[:800], file=sys.stderr)
                return 1
            if i < args.warmup:
                print("warmup {:.3f}s".format(elapsed))
            else:
                timings.append(elapsed)
                print("run {}: {:.3f}s".format(i - args.warmup + 1, elapsed))
    finally:
        shutil.rmtree(temp, ignore_errors=True)

    if not timings:
        return 1
    ordered = sorted(timings)
    print("min={:.3f}s median={:.3f}s avg={:.3f}s ({} runs)".format(
        ordered[0],
        ordered[len(ordered) // 2],
        sum(ordered) / len(ordered),
        len(ordered),
    ))
    return 0


if __name__ == "__main__":
    sys.exit(main())
