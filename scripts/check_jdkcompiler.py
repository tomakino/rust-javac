#!/usr/bin/env python3
"""jdk.compiler parity check: compile the patched jdk.compiler module sources with
javac and rust-javac, then compare the produced class files by count and SHA-256.

The corpus is `internal/reference/jdk17u/src/jdk.compiler/share/classes`, built with
`--patch-module jdk.compiler=<sources>` so the patched sources replace the module
shipped with the JDK. Three resource classes (CompilerProperties, JavacProperties,
LauncherProperties) are generated from the .properties files by the JDK's
CompileProperties tool; `--gen` (re)builds them into target/tmp/jdkcompiler/gen.

Both compilers write into `target/tmp/jdkcompiler/`:

    target/tmp/jdkcompiler/javac.args   javac argument file
    target/tmp/jdkcompiler/rust.args    rust-javac argument file
    target/tmp/jdkcompiler/javac-out/   javac class files
    target/tmp/jdkcompiler/rust-out/    rust-javac class files

The output directories are kept after the run so the class files can be inspected
with javap or dump_stackmap; pass --clean to remove them first.

Usage:
    python scripts/check_jdkcompiler.py [--build] [--gen] [--verbose] [--clean]

Exit codes: 0 identical, 1 differences, 2 setup missing.
"""

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

MODULE = "jdk.compiler"
CORPUS = Path("internal") / "reference" / "jdk17u" / "src" / "jdk.compiler" / "share" / "classes"
RESOURCES = CORPUS / "com" / "sun" / "tools" / "javac" / "resources"
TOOLS = Path("internal") / "reference" / "jdk17u" / "make" / "langtools" / "tools"
PROPERTIES_PARSER = TOOLS / "propertiesparser" / "PropertiesParser.java"
# (properties file, generated class name)
GENERATED = [
    ("compiler.properties", "CompilerProperties"),
    ("javac.properties", "JavacProperties"),
    ("launcher.properties", "LauncherProperties"),
]


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


def write_args(path: Path, sources: list, out_dir: Path) -> None:
    """Write a javac argument file; paths are repository-relative because both
    compilers run with the repository root as their working directory."""
    lines = [
        "--patch-module",
        "{}={}".format(MODULE, CORPUS.as_posix()),
        "-d",
        out_dir.as_posix(),
    ]
    lines.extend(sources)
    path.write_text("\n".join(lines), encoding="utf-8")


def generate_resources(root: Path, gen_dir: Path, javac: str, java: str, tmp: Path) -> int:
    """Run the JDK's PropertiesParser over the .properties files, producing the
    nested Errors/Warnings/Notes/Fragments factories the corpus imports.
    Returns 0 on success."""
    parser_src = root / PROPERTIES_PARSER
    if not parser_src.is_file():
        print("PropertiesParser.java not found: {}".format(parser_src), file=sys.stderr)
        return 2
    if not java:
        print("java not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2
    tools_out = tmp / "tools"
    shutil.rmtree(tools_out, ignore_errors=True)
    tools_out.mkdir(parents=True, exist_ok=True)
    tools_src = root / TOOLS
    proc = subprocess.run(
        [javac, "-d", str(tools_out), "-sourcepath", str(tools_src), str(parser_src)],
        cwd=str(root),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        print("PropertiesParser build failed:", file=sys.stderr)
        print(proc.stderr.decode("utf-8", "replace").splitlines()[:5], file=sys.stderr)
        return 2
    shutil.rmtree(gen_dir, ignore_errors=True)
    out_dir = gen_dir / "com" / "sun" / "tools" / "javac" / "resources"
    out_dir.mkdir(parents=True, exist_ok=True)
    classpath = os.pathsep.join([str(tools_out), str(tools_src)])
    for prop, cls in GENERATED:
        src = root / RESOURCES / prop
        out = out_dir / (cls + ".java")
        proc = subprocess.run(
            [java, "-cp", classpath, "propertiesparser.PropertiesParser",
             "-compile", str(src), str(out_dir)],
            cwd=str(root),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if proc.returncode != 0 or not out.is_file():
            print("PropertiesParser failed for {}:".format(prop), file=sys.stderr)
            print(proc.stderr.decode("utf-8", "replace").splitlines()[:5], file=sys.stderr)
            return 2
        print("generated {}".format(out.relative_to(root).as_posix()))
    return 0


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="cargo build -p rust-javac first")
    parser.add_argument("--gen", action="store_true", help="(re)generate the resources classes first")
    parser.add_argument("--verbose", action="store_true", help="print hashes of differing classes")
    parser.add_argument("--clean", action="store_true", help="remove the output directories first")
    args = parser.parse_args()

    suffix = ".exe" if os.name == "nt" else ""
    rust = root / "target" / "debug" / ("ravac" + suffix)
    javac = resolve_tool("javac", "JAVA_HOME")
    java = resolve_tool("java", "JAVA_HOME")
    corpus = root / CORPUS
    tmp = root / "target" / "tmp" / "jdkcompiler"
    gen_dir = tmp / "gen"
    j_out = tmp / "javac-out"
    r_out = tmp / "rust-out"

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not corpus.is_dir():
        print("corpus not found: {}".format(corpus), file=sys.stderr)
        return 2
    if not rust.is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2
    if not javac:
        print("javac not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2

    tmp.mkdir(parents=True, exist_ok=True)
    if args.gen or not gen_dir.is_dir():
        status = generate_resources(root, gen_dir, javac, java, tmp)
        if status != 0:
            return status

    t_start = time.perf_counter()
    t_scan = time.perf_counter()
    sources = sorted(p.relative_to(root).as_posix() for p in corpus.rglob("*.java"))
    sources += sorted(p.relative_to(root).as_posix() for p in gen_dir.rglob("*.java"))
    t_scan = time.perf_counter() - t_scan
    print("corpus sources={} generated={}".format(
        len(sources) - len(list(gen_dir.rglob("*.java"))), len(list(gen_dir.rglob("*.java")))
    ))
    if not sources:
        print("no java sources under {}".format(corpus), file=sys.stderr)
        return 2

    if args.clean:
        for out_dir in (j_out, r_out):
            shutil.rmtree(out_dir, ignore_errors=True)
    j_out.mkdir(exist_ok=True)
    r_out.mkdir(exist_ok=True)
    write_args(tmp / "javac.args", sources, Path("target") / "tmp" / "jdkcompiler" / "javac-out")
    write_args(tmp / "rust.args", sources, Path("target") / "tmp" / "jdkcompiler" / "rust-out")

    # Both compilers are independent processes writing to separate directories,
    # so they run at the same time; the wall clock is the slower of the two.
    t_parallel = time.perf_counter()
    p_javac = subprocess.Popen(
        [javac, "@" + (tmp / "javac.args").as_posix()],
        cwd=str(root),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    t_rust = time.perf_counter()
    p_rust = subprocess.Popen(
        [str(rust), "@" + (tmp / "rust.args").as_posix()],
        cwd=str(root),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    r_out_bytes, r_err_bytes = p_rust.communicate()
    t_rust = time.perf_counter() - t_rust
    j_out_bytes, j_err_bytes = p_javac.communicate()
    t_parallel = time.perf_counter() - t_parallel

    j_err = j_err_bytes.decode("utf-8", "replace")
    r_err = r_err_bytes.decode("utf-8", "replace")
    j_code = p_javac.returncode
    r_code = p_rust.returncode

    print("javac exit={} rust exit={}".format(j_code, r_code))
    print("timing: javac={:.2f}s rust={:.2f}s ratio={:.2f}x".format(
        t_parallel,
        t_rust,
        (t_parallel / t_rust) if t_rust > 0 else 0.0,
    ))
    if args.verbose:
        for label, err in (("javac", j_err), ("rust", r_err)):
            if err:
                print("{} stderr (head): {}".format(label, err.splitlines()[0]))

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
    if missing:
        print("missing in rust: {}".format(len(missing)))
        for name in missing[:20]:
            print("  MISSING " + name)
    if extra:
        print("extra in rust: {}".format(len(extra)))
        for name in extra[:20]:
            print("  EXTRA " + name)
    if differing:
        print("sha256 differs: {}".format(len(differing)))
        for name in differing[:20]:
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
    identical = len(j_classes) - len(missing) - len(extra) - len(differing)
    t_total = time.perf_counter() - t_start
    print("breakdown: scan={:.2f}s compile={:.2f}s (rust={:.2f}s) hash={:.2f}s total={:.2f}s".format(
        t_scan, t_parallel, t_rust, t_hash, t_total
    ))
    print("identical: {}/{}".format(identical, len(j_classes)))
    print("result: {}".format("PASS" if ok else "FAIL"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
