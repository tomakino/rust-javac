#!/usr/bin/env python3
"""Select the minimal verification gate for the current change set.

Levels (see docs/STYLE.md):
  none  no code changed (docs only)
  L0    cargo test -p javac-core --lib
  L1    L0 + repros + gson              (~15s)
  L2    L1 + guava + okhttp             (~50s)
  L3    everything (feature/cli/examples + all above)  (merge points only)

Known failures: repros and the diagnostic corpus have accepted, tracked
failures (environment-bound repros, module-system diagnostics, PN-001). The
gate accepts those and fails on any new name.

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

RULES: list[tuple[str, str]] = [
    (r"^(docs/|.*\.md$|[^/]+\.md$)", "none"),
    (r"^(crates/[^/]+/tests/|tests/|.*_test\.rs$)", "L0"),
    (r"^crates/javac-core/src/comp/(infer|attr|resolve|check|type_enter|annotate)\.rs$", "L2"),
    (r"^crates/javac-core/src/code/(types|type_|symbol)\.rs$", "L2"),
    (r"^crates/javac-core/src/comp/(lower|trans_types|trans_patterns|lambda_to_method)\.rs$", "L2"),
    (r"^crates/javac-core/src/jvm/", "L2"),
    (r"^crates/javac-core/src/parser/", "L1"),
    (r"^crates/javac-core/src/(main|launcher)/|option\.rs$", "L1"),
    (r"^crates/javac-core/src/", "L1"),
    (r"^crates/javac-", "L1"),
]

ORDER = {"none": 0, "L0": 1, "L1": 2, "L2": 3, "L3": 4}

COMMANDS: dict[str, list[list[str]]] = {
    "none": [],
    "L0": [["cargo", "test", "-p", "javac-core", "--lib"]],
    "L1": [
        ["cargo", "test", "-p", "javac-core", "--lib"],
        [sys.executable, "scripts/check_repros.py"],
        [sys.executable, "scripts/check_gson.py"],
    ],
    "L2": [
        ["cargo", "test", "-p", "javac-core", "--lib"],
        [sys.executable, "scripts/check_repros.py"],
        [sys.executable, "scripts/check_gson.py"],
        [sys.executable, "scripts/check_jdkcompiler.py"],
        [sys.executable, "scripts/check_guava.py"],
        [sys.executable, "scripts/check_okhttp.py"],
    ],
    "L3": [
        # Every crate's unit tests plus the rust-javac integration tests.
        ["cargo", "test", "--workspace", "--no-fail-fast"],
        [sys.executable, "scripts/check_feature_matrix.py"],
        [sys.executable, "scripts/check_cli.py"],
        [sys.executable, "scripts/check_repros.py"],
        [sys.executable, "scripts/check_gson.py"],
        [sys.executable, "scripts/check_jdkcompiler.py"],
        [sys.executable, "scripts/check_guava.py"],
        [sys.executable, "scripts/check_okhttp.py"],
        [sys.executable, "scripts/check_commons_lang3.py"],
        [sys.executable, "scripts/check_junit5.py"],
        [sys.executable, "scripts/check_java_base.py"],
        [sys.executable, "scripts/check_examples.py"],
    ],
}

KNOWN_FAILURES: dict[str, set[str]] = {}


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


def accepted_failures(script: str, output: str) -> tuple[bool, list[str]]:
    known = KNOWN_FAILURES.get(script)
    if known is None:
        return False, []
    names: list[str] = []
    if script == "check_repros.py":
        for line in output.splitlines():
            if line.startswith("failed:"):
                names = [n.strip() for n in line[len("failed:"):].split(",") if n.strip()]
                break
    elif script == "check_examples.py":
        prefix = "DIAG_EXAMPLES FAIL "
        names = [line[len(prefix):].strip() for line in output.splitlines() if line.startswith(prefix)]
    unknown = [n for n in names if n not in known]
    return (len(unknown) == 0 and len(names) > 0), sorted(set(names) - known)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", help="execute the selected commands")
    parser.add_argument("--files", nargs="*", default=None, help="explicit file list")
    parser.add_argument("--level", choices=list(ORDER), default=None, help="force a level")
    parser.add_argument(
        "--keep-going",
        action="store_true",
        help="run every command even after a failure and report all failures",
    )
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

    failures: list[tuple[int, str]] = []
    for cmd in commands:
        print(f"\n>>> {' '.join(cmd)}", flush=True)
        result = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        print(result.stdout, end="")
        if result.returncode == 0:
            continue
        script = Path(cmd[-1]).name
        ok, unknown = accepted_failures(script, result.stdout)
        if ok:
            print(f"accepted known failures: {', '.join(unknown) if unknown else 'none new'}")
            continue
        print(result.stderr, end="", file=sys.stderr)
        print(f"FAILED ({result.returncode}): {' '.join(cmd)}", file=sys.stderr)
        if not args.keep_going:
            return result.returncode
        failures.append((result.returncode, " ".join(cmd)))
    if failures:
        print(f"\n{len(failures)} command(s) failed:", file=sys.stderr)
        for code, text in failures:
            print(f"  FAILED ({code}): {text}", file=sys.stderr)
        return failures[0][0]
    print("\nall gates passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
