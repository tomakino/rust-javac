#!/usr/bin/env python3
"""JUnit 5 parity check.

Compiles every self-contained JUnit 5 module (r5.10.2) with the reference
javac and with rust-javac, then compares the emitted class files by relative
name and SHA-256.  Modules that need third-party artifacts which are not
vendored in this tree are skipped explicitly; a module that neither compiler
accepts is kept as long as both report the same first diagnostic.

Usage:
    python scripts/check_junit5.py [--build] [--verbose] [--module NAME]

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

# Modules whose src/main/java cannot be compiled from this tree alone.
SKIP_MODULES = {
    "junit-platform-reporting": "needs org.opentest4j.reporting",
    "platform-tests": "needs org.opentest4j.reporting",
    "junit-jupiter-migrationsupport": "needs JUnit 4",
    "junit-platform-runner": "needs JUnit 4",
    "junit-vintage-engine": "needs JUnit 4",
    "documentation": "documentation examples, not a library module",
    "platform-tooling-support-tests": "test fixtures, not a library module",
}


def resolve_tool(name, env_name, fallback=None):
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


def hash_dir(root):
    result = {}
    if not root.is_dir():
        return result
    for path in sorted(root.rglob("*.class")):
        if path.is_file():
            rel = path.relative_to(root).as_posix()
            result[rel] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def first_error(text):
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("error:") or ": error:" in line:
            return line
    for line in text.splitlines():
        if line.strip():
            return line.strip()
    return ""


def run(exe, opts, cwd):
    started = time.perf_counter()
    proc = subprocess.run(
        [exe] + opts,
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    elapsed = time.perf_counter() - started
    out = proc.stdout.decode("utf-8", "replace")
    err = proc.stderr.decode("utf-8", "replace")
    return proc.returncode, out, err, elapsed


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description="JUnit 5 parity check")
    parser.add_argument("--build", action="store_true", help="cargo build -p rust-javac first")
    parser.add_argument("--verbose", action="store_true", help="show the first differing classes")
    parser.add_argument("--module", action="append", help="only check these modules (repeatable)")
    args = parser.parse_args()

    projects = root / "internal" / "test" / "real-projects"
    junit_root = projects / "junit-framework-r5.10.2"
    deps_root = projects / "junit5-deps"
    suffix = ".exe" if os.name == "nt" else ""
    rust = root / "target" / "debug" / ("rust-javac" + suffix)
    javac = resolve_tool("javac", "JAVA_HOME")

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not junit_root.is_dir():
        print("junit5 sources not found: {}".format(junit_root), file=sys.stderr)
        return 2
    if not rust.is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2
    if not javac:
        print("javac not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2

    stub_src = deps_root / "src"
    stub_files = sorted(str(p) for p in stub_src.rglob("*.java")) if stub_src.is_dir() else []

    modules = []
    for module_src in sorted(junit_root.glob("*/src/main/java")):
        name = module_src.parent.parent.parent.name
        sources = sorted(str(p) for p in module_src.rglob("*.java"))
        if sources:
            modules.append((name, sources))

    checked = [m for m in modules if m[0] not in SKIP_MODULES]
    skipped = [m for m in modules if m[0] in SKIP_MODULES]
    if args.module:
        wanted = set(args.module)
        checked = [m for m in checked if m[0] in wanted]

    print("junit5 modules: {} checked, {} skipped, {} stub sources".format(
        len(checked), len(skipped), len(stub_files)))
    for name, _ in skipped:
        print("  SKIP {} ({})".format(name, SKIP_MODULES[name]))

    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    temp = Path(tempfile.mkdtemp(prefix="junit5-", dir=str(tmp_root)))
    common = ["-J-Duser.language=en", "-J-Duser.country=US"]
    total_classes = 0
    failures = 0
    t_start = time.perf_counter()
    try:
        # Compile the tiny apiguardian / opentest4j stand-ins once; both
        # compilers then read the very same class files from the class path.
        stub_out = temp / "stubs"
        stub_out.mkdir()
        if stub_files:
            code, out, err, _ = run(javac, common + ["-d", str(stub_out)] + stub_files, temp)
            if code != 0:
                print("stub compilation failed:\n" + err, file=sys.stderr)
                return 2
        stub_cp = [str(stub_out)] if stub_files else []

        results = {}
        pending = list(checked)
        done = []
        while pending:
            progressed = False
            still = []
            # A module that javac accepts still contributes its class files to
            # the class path of the modules that depend on it, even when
            # rust-javac failed on it: that keeps one broken module from
            # masking every module downstream of it.
            for name, sources in pending:
                classpath = list(stub_cp)
                classpath += [str(temp / "javac" / m) for m in done]
                for compiler in ("javac", "rust"):
                    (temp / compiler / name).mkdir(parents=True, exist_ok=True)
                cp_args = ["--class-path", os.pathsep.join(classpath)] if classpath else []

                j_code, j_out, j_err, j_time = run(
                    javac,
                    common + ["-d", str(temp / "javac" / name)] + cp_args + sources,
                    temp)
                r_code, r_out, r_err, r_time = run(
                    str(rust),
                    common + ["-d", str(temp / "rust" / name)] + cp_args + sources,
                    temp)

                if j_code == 0:
                    done.append(name)
                    progressed = True
                if j_code == 0 and r_code == 0:
                    results[name] = ("ok", hash_dir(temp / "javac" / name),
                                     hash_dir(temp / "rust" / name), j_time, r_time)
                elif j_code == 0:
                    results[name] = ("rust-fails", r_code, first_error(r_err))
                elif r_code == 0:
                    results[name] = ("rust-accepts", j_code, first_error(j_err))
                else:
                    results[name] = ("both-fail", first_error(j_err), first_error(r_err))
                    still.append((name, sources))
            if not progressed:
                break
            pending = still

        for name, _ in checked:
            res = results.get(name)
            if res is None:
                continue
            if res[0] == "ok":
                j_classes, r_classes = res[1], res[2]
                total_classes += len(j_classes)
                missing = sorted(set(j_classes) - set(r_classes))
                extra = sorted(set(r_classes) - set(j_classes))
                differing = sorted(n for n in set(j_classes) & set(r_classes)
                                   if j_classes[n] != r_classes[n])
                ok = not (missing or extra or differing)
                if not ok:
                    failures += 1
                print("[{}] {:<32} classes={:<5} javac={:.2f}s rust={:.2f}s".format(
                    "OK  " if ok else "FAIL", name, len(j_classes), res[3], res[4]))
                if missing:
                    print("    missing in rust: {}".format(len(missing)))
                    for n in missing[:5]:
                        print("      MISSING " + n)
                if extra:
                    print("    extra in rust: {}".format(len(extra)))
                    for n in extra[:5]:
                        print("      EXTRA " + n)
                if differing:
                    print("    sha256 differs: {}".format(len(differing)))
                    for n in differing[:5]:
                        print("      DIFF " + n)
            elif res[0] == "both-fail":
                j_msg, r_msg = res[1], res[2]
                if j_msg == r_msg and j_msg:
                    print("[ERR ] {:<32} both reject: {}".format(name, j_msg[:110]))
                else:
                    failures += 1
                    print("[FAIL] {:<32} diagnostics differ".format(name))
                    print("    javac: " + j_msg[:200])
                    print("    rust : " + r_msg[:200])
            elif res[0] == "rust-fails":
                failures += 1
                print("[FAIL] {:<32} javac accepts, rust exits {}".format(name, res[1]))
                print("    " + res[2][:200])
            else:
                failures += 1
                print("[FAIL] {:<32} rust accepts, javac exits {}".format(name, res[1]))
                print("    " + res[2][:200])

        elapsed = time.perf_counter() - t_start
        print("checked classes={} failures={} total={:.1f}s".format(
            total_classes, failures, elapsed))
        if failures:
            print("junit5 parity: FAIL")
            return 1
        print("junit5 parity: OK")
        return 0
    finally:
        shutil.rmtree(str(temp), ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
