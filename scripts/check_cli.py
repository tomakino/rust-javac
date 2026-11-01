#!/usr/bin/env python3
"""CLI behaviour parity check: compare javac and rust-javac on the observable
command-line contract (version, exit codes, help markers, -Xstdout).

Usage:
    python scripts/check_cli.py [--build]

Exit codes: 0 all checks pass, 1 differences, 2 setup missing.
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

CHECKS = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, ok, detail))
    print("[{}] {}{}".format("PASS" if ok else "FAIL", name, (" - " + detail) if detail else ""))


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
    args = parser.parse_args()

    suffix = ".exe" if os.name == "nt" else ""
    rust = root / "target" / "debug" / ("rust-javac" + suffix)
    home = os.environ.get("JAVA_HOME")
    javac = str(Path(home) / "bin" / ("javac" + suffix)) if home else ""
    if not javac or not Path(javac).is_file():
        javac = shutil.which("javac") or ""

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not rust.is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2
    if not javac:
        print("javac not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2

    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    temp = Path(tempfile.mkdtemp(prefix="cli-", dir=str(tmp_root)))
    try:
        # 1. --version
        j_out, _, j_code = run(javac, ["--version"], temp)
        r_out, _, r_code = run(str(rust), ["--version"], temp)
        check(
            "--version",
            j_code == r_code and j_out.strip() == r_out.strip(),
            "javac={!r} rust={!r}".format(j_out.strip(), r_out.strip()),
        )

        # 2. no arguments
        j_out, j_err, j_code = run(javac, [], temp)
        r_out, r_err, r_code = run(str(rust), [], temp)
        check(
            "no-arguments exit code",
            j_code == r_code,
            "javac={} rust={}".format(j_code, r_code),
        )

        # 3. unknown option
        j_out, j_err, j_code = run(javac, ["--not-an-option"], temp)
        r_out, r_err, r_code = run(str(rust), ["--not-an-option"], temp)
        check(
            "unknown-option exit code",
            j_code == r_code,
            "javac={} rust={}".format(j_code, r_code),
        )

        # 4. help contains the -J line exactly once
        j_out, j_err, _ = run(javac, ["--help"], temp)
        r_out, r_err, _ = run(str(rust), ["--help"], temp)
        j_help = j_out + j_err
        r_help = r_out + r_err
        j_lines = [line for line in j_help.splitlines() if line.strip().startswith("-J<flag>")]
        r_lines = [line for line in r_help.splitlines() if line.strip().startswith("-J<flag>")]
        check(
            "--help -J<flag> line",
            len(j_lines) >= 1 and len(r_lines) == 1,
            "javac={} rust={}".format(len(j_lines), len(r_lines)),
        )

        # 5. -Xstdout redirects diagnostics to a file and keeps the console quiet
        source = temp / "X.java"
        source.write_text("class X { void f() { int i = 0; } }\n", encoding="utf-8")
        results = {}
        for label, exe in (("javac", javac), ("rust", str(rust))):
            out_file = temp / (label + "-xstdout.txt")
            out_dir = temp / (label + "-out")
            out_dir.mkdir()
            stdout, stderr, code = run(
                exe,
                ["-J-Duser.language=en", "-J-Duser.country=US", "-Xstdout", str(out_file), "-d", str(out_dir), str(source)],
                temp,
            )
            results[label] = (out_file.read_text(encoding="utf-8", errors="replace") if out_file.is_file() else "", stdout, stderr, code)
        j_file, j_stdout, j_stderr, j_code = results["javac"]
        r_file, r_stdout, r_stderr, r_code = results["rust"]
        check(
            "-Xstdout file content",
            j_code == r_code and j_file.strip() == r_file.strip() and j_stdout.strip() == r_stdout.strip(),
            "javac={!r} rust={!r}".format(j_file.strip()[:60], r_file.strip()[:60]),
        )

        failed = [name for name, ok, _ in CHECKS if not ok]
        print("")
        print("cli summary: {}/{} passed, {} failed".format(len(CHECKS) - len(failed), len(CHECKS), len(failed)))
        return 0 if not failed else 1
    finally:
        shutil.rmtree(temp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
