#!/usr/bin/env python3
"""Language feature matrix parity check.

Every unit under internal/test/feature-matrix/<release>/<case>/ is compiled by
javac and rust-javac with identical arguments; the exit code, the normalized
stdout+stderr, the class file count and every class SHA-256 must match.

Usage:
    python scripts/check_feature_matrix.py [--build] [--filter SUBSTR] [--verbose]

Exit codes: 0 all units match, 1 differences, 2 setup missing.
"""

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def resolve_tool(name: str, env_name: str) -> str:
    suffix = ".exe" if os.name == "nt" else ""
    home = os.environ.get(env_name)
    if home:
        candidate = Path(home) / "bin" / (name + suffix)
        if candidate.is_file():
            return str(candidate)
    return shutil.which(name) or ""


def hash_dir(root: Path) -> dict:
    result = {}
    if not root.is_dir():
        return result
    for path in sorted(root.rglob("*.class")):
        rel = path.relative_to(root).as_posix()
        result[rel] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def normalize(text: str, temp: Path, unit_dir: Path) -> str:
    for base in (temp, unit_dir):
        raw = str(base)
        unix = raw.replace("\\", "/")
        text = text.replace(raw + "\\", "<tmp>/").replace(unix + "/", "<tmp>/")
        text = text.replace(raw, "<tmp>").replace(unix, "<tmp>")
    return text.replace("\r\n", "\n")


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


def collect_units(matrix_root: Path):
    units = []
    for release in sorted(matrix_root.iterdir()):
        if not release.is_dir():
            continue
        for case in sorted(release.iterdir()):
            if not case.is_dir():
                continue
            sources = sorted(str(p) for p in case.rglob("*.java"))
            if not sources:
                continue
            is_module = any(Path(p).name == "module-info.java" for p in sources)
            units.append((release.name + "/" + case.name, case, sources, is_module))
    return units


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="cargo build -p rust-javac first")
    parser.add_argument("--filter", default="", help="only run units whose name contains this")
    parser.add_argument("--verbose", action="store_true", help="print the first diagnostic line per failure")
    args = parser.parse_args()

    matrix_root = root / "internal" / "test" / "feature-matrix"
    suffix = ".exe" if os.name == "nt" else ""
    rust = root / "target" / "debug" / ("ravac" + suffix)
    javac = resolve_tool("javac", "JAVA_HOME")

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not matrix_root.is_dir():
        print("matrix not found: {}".format(matrix_root), file=sys.stderr)
        return 2
    if not rust.is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2
    if not javac:
        print("javac not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2

    units = collect_units(matrix_root)
    if args.filter:
        units = [unit for unit in units if args.filter in unit[0]]
    print("units={}".format(len(units)))

    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    failures = 0
    for name, unit_dir, sources, is_module in units:
        temp = Path(tempfile.mkdtemp(prefix="matrix-", dir=str(tmp_root)))
        try:
            j_out = temp / "javac"
            r_out = temp / "rust"
            j_out.mkdir()
            r_out.mkdir()
            prefix = ["-J-Duser.language=en", "-J-Duser.country=US"]
            extra = []
            if is_module:
                extra += ["--module-source-path", str(unit_dir / "src")]
            for out_dir in (j_out, r_out):
                lines = ["-d", str(out_dir)] + extra + sources
                (temp / (out_dir.name + ".args")).write_text("\n".join(lines), encoding="utf-8")
            j_stdout, j_stderr, j_code = run(javac, prefix + ["@" + str(temp / "javac.args")], temp)
            r_stdout, r_stderr, r_code = run(str(rust), prefix + ["@" + str(temp / "rust.args")], temp)

            j_classes = hash_dir(j_out)
            r_classes = hash_dir(r_out)
            missing = sorted(set(j_classes) - set(r_classes))
            extra_classes = sorted(set(r_classes) - set(j_classes))
            differing = sorted(
                key for key in set(j_classes) & set(r_classes) if j_classes[key] != r_classes[key]
            )
            j_diag = normalize(j_stdout + j_stderr, temp, unit_dir).strip()
            r_diag = normalize(r_stdout + r_stderr, temp, unit_dir).strip()
            diag_ok = j_diag == r_diag
            ok = (
                j_code == r_code
                and diag_ok
                and not missing
                and not extra_classes
                and not differing
            )
            status = "PASS" if ok else "FAIL"
            detail = "classes={} sha_diff={} exit={}/{} diag={}".format(
                len(r_classes), len(differing), j_code, r_code, "same" if diag_ok else "diff"
            )
            print("[{}] {} - {}".format(status, name, detail))
            if not ok:
                failures += 1
                if args.verbose:
                    if not diag_ok:
                        j_first = j_diag.splitlines()[0] if j_diag else "<empty>"
                        r_first = r_diag.splitlines()[0] if r_diag else "<empty>"
                        print("    javac: " + j_first)
                        print("    rust : " + r_first)
                    for label, items in (("missing", missing), ("extra", extra_classes), ("sha", differing)):
                        for item in items[:3]:
                            print("    {}: {}".format(label, item))
        finally:
            shutil.rmtree(temp, ignore_errors=True)

    print("")
    print("feature-matrix summary: {}/{} passed, {} failed".format(len(units) - failures, len(units), failures))
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
