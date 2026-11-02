#!/usr/bin/env python3
"""Select the minimal verification gate for the current change set.

Levels (see docs/STYLE.md):
  none  no code changed (docs only)
  L0    cargo test -p javac-core --lib
  L1    L0 + repros + gson              (~15s)
  L2    L1 + guava + okhttp             (~50s)
  L3    everything (feature/cli/examples + all above)  (merge points only)

Usage:
  py scripts/gate.py                 # print the plan for the current diff
  py scripts/gate.py --run           # run the selected commands, stop on first failure
  py scripts/gate.py --files a.rs b.rs
  py scripts/gate.py --level L2 --run   # force a level
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# path regex -> minimum level; the highest matching level wins.
RULES: list[tuple[str, str]] = [
    (r"^(docs/|.*\.md$|[^/]+\.md$)", "none"),
    (r"^(crates/[^/]+/tests/|tests/|.*_test\.rs$)", "L0"),
    # semantic core: type/inference/attribution changes are the most sensitive
    (r"^crates/javac-core/src/comp/(infer|attr|resolve|check|type_enter|annotate)\.rs$", "L2"),
    (r"^crates/javac-core/src/code/(types|type_|symbol)\.rs$", "L2"),
    (r"^crates/javac-core/src/comp/(lower|trans_types|trans_patterns|lambda_to_method)\.rs$", "L2"),
    (r"^crates/javac-core/src/jvm/", "L2"),
    # parser / diagnostics / driver / CLI: fast feedback is enough
    (r"^crates/javac-core/src/parser/", "L1"),
    (r"^crates/javac-core/src/(main|launcher)/|option\.rs$", "L1"),
    (r"^crates/javac-core/src/", "L1"),
    (r"^crates/javac-", "L1"),
]

ORDER = {"none": 0, "L0": 1, "L1": 2, "L2": 3, "L3": 4}

COMMANDS: dict[str, list[list[str]]] = {
    "none": [],
    "L0": [
        ["cargo", "test", "-p", "javac-core", "--lib"],
    ],
    "L1": [
        ["cargo", "test", "-p", "javac-core", "--lib"],
        [sys.executable, "scripts/check_repros.py"],
        [sys.executable, "scripts/check_gson.py"],
    ],
    "L2": [
        ["cargo", "test", "-p", "javac-core", "--lib"],
        [sys.executable, "scripts/check_repros.py"],
        [sys.executable, "scripts/check_gson.py"],
        [sys.executable, "scripts/check_guava.py"],
        [sys.executable, "scripts/check_okhttp.py"],
    ],
    "L3": [
        ["cargo", "test", "-p", "javac-core", "--lib"],
        [sys.executable, "scripts/check_feature_matrix.py"],
        [sys.executable, "scripts/check_cli.py"],
        [sys.executable, "scripts/check_repros.py"],
        [sys.executable, "scripts/check_gson.py"],
        [sys.executable, "scripts/check_guava.py"],
        [sys.executable, "scripts/check_okhttp.py"],
        [sys.executable, "scripts/check_examples.py"],
    ],
}


def diff_files() -> list[str]:
    out = subprocess.run(
        ["git", "diff", "--name-only", "HEAD"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    files = [line.strip().replace("\\", "/") for line in (out + "\n" + untracked).splitlines()]
    return sorted({f for f in files if f})


def classify(files: list[str]) -> tuple[str, list[str]]:
    level = "none"
    hits: list[str] = []
    for path in files:
        for pattern, candidate in RULES:
            if re.search(pattern, path):
                if ORDER[candidate] > ORDER[level]:
                    level = candidate
                hits.append(f"{path} -> {candidate}")
                break
    return level, hits


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", help="execute the selected commands")
    parser.add_argument("--files", nargs="*", default=None, help="explicit file list")
    parser.add_argument("--level", choices=list(ORDER), default=None, help="force a level")
    args = parser.parse_args()

    files = args.files if args.files is not None else diff_files()
    level, hits = classify(files)
    if args.level is not None:
        level = args.level

    print(f"files: {len(files)}")
    for hit in hits:
        print(f"  {hit}")
    print(f"gate: {level}")

    commands = COMMANDS[level]
    if not commands:
        print("nothing to run (docs-only change)")
        return 0

    print("commands:")
    for cmd in commands:
        print("  " + " ".join(cmd))

    if not args.run:
        return 0

    for cmd in commands:
        print(f"\n>>> {' '.join(cmd)}", flush=True)
        result = subprocess.run(cmd, cwd=ROOT)
        if result.returncode != 0:
            print(f"FAILED ({result.returncode}): {' '.join(cmd)}", file=sys.stderr)
            return result.returncode
    print("\nall gates passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
