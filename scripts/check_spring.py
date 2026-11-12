#!/usr/bin/env python3
"""Spring Framework parity check: compile the spring-core, spring-beans,
spring-context, spring-aop and spring-expression sources with javac and
rust-javac, then compare the produced class files by count and SHA-256.

The optional integrations that need jars outside ``spring-deps`` (Jakarta
Validation/CDI, Groovy, SnakeYAML, Netty 4/5, Micrometer, GraalVM, CRaC,
Hibernate, BeanShell, jopt-simple, BlockHound) are excluded before compiling.
Everything else must compile.

Usage:
    python scripts/check_spring.py [--build] [--verbose] [--release N]

Exit codes: 0 identical, 1 differences, 2 setup missing.
"""

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ALLOWED = re.compile(
    r"^(?:java\."
    r"|javax\.(?:accessibility|annotation|crypto|imageio|lang|management|naming|net"
    r"|print|script|security|sound|sql|swing|tools|transaction\.xa)\."
    r"|jdk\.|sun\.|com\.sun\.|org\.springframework\."
    r"|org\.apache\.commons\.logging\.|org\.aspectj\.|org\.aopalliance\."
    r"|org\.w3c\.|org\.xml\.|jakarta\.inject\."
    r"|kotlin\.|kotlinx\.|reactor\.|org\.reactivestreams\.|org\.slf4j\.)"
)
IMPORT = re.compile(r"^import\s+(?:static\s+)?([A-Za-z_][\w.]*)")


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


MARKERS = (
    "io.reactivex.",
    "io.netty.",
    "io.micrometer.",
    "org.yaml.snakeyaml",
    "groovy.",
    "jakarta.validation",
    "org.hibernate.",
    "org.crac.",
    "bsh.",
    "joptsimple.",
    "org.graalvm.",
    "reactor.blockhound.",
    "org.osgi.",
    "org.jboss.",
    "org.bouncycastle.",
    "org.conscrypt.",
    "com.aayushatharva.",
    "lzma.",
    "net.jpountz.",
    "com.ning.",
    "org.eclipse.",
    "com.oracle.",
    "com.jcraft.",
    "com.github.",
)


def select_sources(all_sources):
    """Drop sources that reference libraries outside ``spring-deps``.

    Some files use fully qualified names instead of imports (for example
    ``ReactiveAdapterRegistry`` mentions ``io.reactivex.rxjava3.core.Flowable`` in
    method bodies), so the source text is scanned for known foreign packages too.
    Excluding only the direct offenders leaves dangling references, so the
    exclusion is closed over the project's own imports and same-package uses.
    """
    info = {}
    for path in all_sources:
        text = path.read_text(encoding="utf-8", errors="replace")
        package = ""
        imports = []
        for line in text.splitlines():
            if line.startswith("package "):
                package = line[len("package ") :].rstrip(";").strip()
            match = IMPORT.match(line)
            if match:
                imports.append(match.group(1))
        info[path] = (package, imports, text)

    known = {
        (info[path][0] + "." if info[path][0] else "") + path.stem for path in info
    }

    def unresolved(name):
        # A project import is unresolved when the corpus does not ship the class;
        # the last segment may be a nested class, so the parent is checked too.
        if not name.startswith("org.springframework."):
            return False
        return name not in known and name.rsplit(".", 1)[0] not in known

    excluded = {
        path
        for path, (_, imports, text) in info.items()
        if any(not ALLOWED.match(name) or unresolved(name) for name in imports)
        or any(marker in text for marker in MARKERS)
    }
    changed = True
    while changed:
        changed = False
        excluded_names = {
            (info[path][0] + "." if info[path][0] else "") + path.stem for path in excluded
        }
        excluded_by_package = {}
        for path in excluded:
            excluded_by_package.setdefault(info[path][0], set()).add(path.stem)
        for path, (package, imports, _) in info.items():
            if path in excluded:
                continue
            if any(name in excluded_names for name in imports):
                excluded.add(path)
                changed = True
                continue
            # A same-package reference has no import line; the simple name in the
            # body is the only signal.
            names = excluded_by_package.get(package)
            if names:
                text = path.read_text(encoding="utf-8", errors="replace")
                if any(re.search(r"\b" + re.escape(name) + r"\b", text) for name in names):
                    excluded.add(path)
                    changed = True
    kept = [str(path) for path in all_sources if path not in excluded]
    return kept, len(excluded)


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="cargo build -p rust-javac first")
    parser.add_argument("--verbose", action="store_true", help="print the first differing classes")
    parser.add_argument("--release", help="pass --release N to both compilers")
    args = parser.parse_args()

    projects = root / "internal" / "test" / "real-projects"
    spring_root = projects / "spring-6.1.14-src"
    deps_root = projects / "spring-deps"
    suffix = ".exe" if os.name == "nt" else ""
    rust = root / "target" / "debug" / ("ravac" + suffix)
    javac = resolve_tool("javac", "JAVA_HOME")

    if args.build:
        if subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not spring_root.is_dir():
        print("spring sources not found: {}".format(spring_root), file=sys.stderr)
        return 2
    if not rust.is_file():
        print("rust-javac not found: {} (run with --build)".format(rust), file=sys.stderr)
        return 2
    if not javac:
        print("javac not found (set JAVA_HOME or add it to PATH)", file=sys.stderr)
        return 2

    jars = sorted(str(p) for p in deps_root.glob("*.jar")) if deps_root.is_dir() else []
    if not jars:
        print("dependency jars not found below {}".format(deps_root), file=sys.stderr)
        return 2
    classpath = os.pathsep.join(jars)

    t_start = time.perf_counter()
    all_sources = sorted(spring_root.rglob("*.java"))
    sources, excluded = select_sources(all_sources)
    if not sources:
        print("no java sources below {}".format(spring_root), file=sys.stderr)
        return 2
    print("spring sources={} excluded={} jars={}".format(len(sources), excluded, len(jars)))

    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    temp = Path(tempfile.mkdtemp(prefix="spring-", dir=str(tmp_root)))
    try:
        j_out = temp / "javac"
        r_out = temp / "rust"
        j_out.mkdir()
        r_out.mkdir()
        common = [
            "-encoding",
            "UTF-8",
            "-J-Duser.language=en",
            "-J-Duser.country=US",
            "-cp",
            classpath,
        ]
        if args.release:
            common = common + ["--release", args.release]
        for out_dir in (j_out, r_out):
            lines = ["-d", str(out_dir)] + sources
            (temp / (out_dir.name + ".args")).write_text("\n".join(lines), encoding="utf-8")

        if args.verbose:
            print("javac: " + javac + " " + " ".join(common) + " @javac.args")
            print("rust : " + str(rust) + " " + " ".join(common) + " @rust.args")
            print("javac.args: -d {} + {} sources".format(j_out, len(sources)))
            print("rust.args : -d {} + {} sources".format(r_out, len(sources)))

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
            for line in (j_err or "").splitlines()[:5]:
                print("javac stderr: " + line)
            errors = [line for line in (r_err or "").splitlines() if "error" in line.lower()]
            for line in errors[:20]:
                print("rust error: " + line)

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
