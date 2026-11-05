#!/usr/bin/env python3
"""One-shot parity check for the official javac diagnostic examples.

Compiles every example under internal/test/diag-corpus/examples with the
rust-javac binary and compares the normalized stdout/stderr with the frozen
baselines under internal/test/diag-corpus/expected.

Usage:
    python scripts/check_examples.py
    python scripts/check_examples.py --filter Error
    python scripts/check_examples.py --filter Error --verbose
    python scripts/check_examples.py --build

Exit codes: 0 all compared cases match, 1 differences found, 2 setup missing.

Cross-platform: no shell dependency, paths via pathlib, binary resolved with the
platform executable suffix.
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

JAVA_STATEMENT = None
SKIP_OPTIONS = {"-processor", "-proc:only", "-processorpath", "--processor-module-path"}


def strip_spaces(value: str) -> str:
    index = 0
    while index < len(value) and value[index] == " ":
        index += 1
    return value[index:]


def java_statement_line(line: str) -> bool:
    trimmed = strip_spaces(line)
    if not trimmed:
        return False
    first = trimmed[0]
    if first == "@":
        return len(trimmed) > 1 and trimmed[1].isalpha()
    return first.isalpha()


def source_has_declarations(path: Path) -> bool:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return False
    for line in text.split("\n"):
        if java_statement_line(line.rstrip("\r")):
            return True
    return False


def split_words(value: str):
    text = value.strip().replace("" + "$" + "{jdk.version}", "17")
    return [word for word in text.split(" ") if word]


def parse_header(path: Path):
    options = None
    run = None
    text = path.read_text(encoding="utf-8", errors="replace")
    for raw in text.split("\n"):
        line = raw[:-1] if raw.endswith("\r") else raw
        trimmed = strip_spaces(line)
        if trimmed.startswith("//"):
            rest = strip_spaces(trimmed[2:])
            if rest.startswith("options:"):
                options = split_words(rest[len("options:"):])
            elif rest.startswith("run:"):
                run = split_words(rest[len("run:"):])
        if java_statement_line(line):
            break
    return options, run


def find_files(directory: Path, out: list) -> None:
    if directory.is_dir():
        for child in sorted(directory.iterdir(), key=lambda item: item.name):
            find_files(child, out)
    elif directory.is_file() and directory.name.endswith(".java"):
        out.append(directory)


class Example:
    def __init__(self, name: str, entry: Path):
        self.name = name
        self.is_dir = entry.is_dir()
        self.src = []
        self.proc = []
        self.src_path = []
        self.src_path_dir = None
        self.module_source_path = []
        self.module_source_path_dir = None
        self.patch_module = []
        self.patch_module_dir = None
        self.module_path = []
        self.class_path = []
        self.additional = []
        self.options = []
        self.run = []
        if self.is_dir:
            self._collect_dir(entry)
        else:
            self.src.append(entry)
        for source in self.src:
            options, run = parse_header(source)
            if options is not None:
                self.options = options
            if run is not None:
                self.run = run

    def _collect_dir(self, directory: Path) -> None:
        for child in sorted(directory.iterdir(), key=lambda item: item.name):
            name = child.name
            if child.is_dir():
                if name == "processors":
                    find_files(child, self.proc)
                elif name == "sourcepath":
                    self.src_path_dir = child
                    find_files(child, self.src_path)
                elif name == "modulesourcepath":
                    self.module_source_path_dir = child
                    find_files(child, self.module_source_path)
                elif name == "patchmodule":
                    self.patch_module_dir = child
                    find_files(child, self.patch_module)
                elif name == "additional":
                    find_files(child, self.additional)
                elif name == "modulepath":
                    find_files(child, self.module_path)
                elif name == "classpath":
                    find_files(child, self.class_path)
                else:
                    find_files(child, self.src)
            elif name.endswith(".java"):
                self.src.append(child)
            elif name == "modulesourcepath":
                self.module_source_path_dir = child


def skip_reason(example: Example):
    options = example.options
    if example.proc or any(option in SKIP_OPTIONS for option in options):
        return "annotation-processing"
    if "--enable-preview" in options:
        return "preview"
    if "--enable-preview" in options:
        return "preview"
    if "-verbose" in options:
        return "verbose"
    if "-doe" in options:
        return "jvm-stacktrace"
    if "-profile" in options:
        return "deprecated-profile"
    if "-source" in options:
        index = options.index("-source")
        if index + 1 < len(options) and options[index + 1] == "7":
            return "deprecated-source"
    if example.module_path and not any(
        path.name == "module-info.java" for path in example.module_path
    ):
        return "modules-automatic"
    if "-filemanager" in example.run:
        return "custom-file-manager"
    if any(option == "-Xplugin" or option.startswith("-Xplugin:") for option in options):
        return "plugin"
    return None


def normalize(text: str, examples_dir: Path, temp: Path) -> str:
    base = str(examples_dir)
    base_unix = base.replace("\\", "/")
    tmp = str(temp)
    tmp_unix = tmp.replace("\\", "/")
    result = text.replace("\r\n", "\n")
    result = result.replace(base + "\\", "<src>/")
    result = result.replace(base_unix + "/", "<src>/")
    result = result.replace(base, "<src>")
    result = result.replace(base_unix, "<src>")
    result = result.replace(tmp, "<tmp>")
    result = result.replace(tmp_unix, "<tmp>")
    return result


def run_compiler(exe: str, args: list, cwd: Path, env):
    command = [exe, "-J-Duser.language=en", "-J-Duser.country=US"] + args
    proc = subprocess.run(
        command,
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    stdout = proc.stdout.decode("utf-8", errors="replace")
    stderr = proc.stderr.decode("utf-8", errors="replace")
    return stdout, stderr, proc.returncode


def run_example(example: Example, temp: Path, exe: str, examples_dir: Path, env) -> str:
    classes = temp / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    args = []
    prefix_errors = ""

    if example.class_path:
        classpath_dir = temp / "classpath"
        classpath_dir.mkdir(parents=True, exist_ok=True)
        cp_args = ["-d", str(classpath_dir)] + [str(item) for item in example.class_path]
        _, cp_err, cp_code = run_compiler(exe, cp_args, temp, env)
        if cp_code != 0:
            prefix_errors += cp_err
        args += ["--class-path", str(classpath_dir)]

    args += ["-d", str(classes)]
    args += example.options

    if example.additional:
        extra = ["-d", str(classes)] + [str(item) for item in example.additional]
        run_compiler(exe, extra, temp, env)

    if example.module_path:
        module_root = None
        for item in example.module_path:
            for parent in item.parents:
                if parent.name == "modulepath":
                    module_root = parent
                    break
            if module_root is not None:
                break
        if module_root is None:
            raise RuntimeError("modulepath directory not found")
        module_dir = temp / "modulepath"
        module_dir.mkdir(parents=True, exist_ok=True)
        module_args = ["-d", str(module_dir), "--module-source-path", str(module_root)]
        module_args += [str(item) for item in example.module_path]
        run_compiler(exe, module_args, temp, env)
        args += ["--module-path", str(module_dir)]

    if example.src_path_dir is not None:
        args += ["-sourcepath", str(example.src_path_dir)]
    if example.module_source_path_dir is not None:
        args += ["--module-source-path", str(example.module_source_path_dir)]
    if example.patch_module_dir is not None:
        for patch in sorted(example.patch_module_dir.iterdir(), key=lambda item: item.name):
            if patch.is_dir():
                args += ["--patch-module", patch.name + "=" + str(patch)]

    sources = []
    if example.module_source_path_dir is not None:
        sources += example.module_source_path
        sources += [item for item in example.src if source_has_declarations(item)]
    elif example.patch_module_dir is not None:
        sources += example.patch_module
        sources += [item for item in example.src if source_has_declarations(item)]
    else:
        sources += example.src
    args += [str(item) for item in sources]

    stdout, stderr, _ = run_compiler(exe, args, temp, env)
    return normalize(stdout + stderr + prefix_errors, examples_dir, temp)


def first_diff(actual: list, expected: list) -> str:
    for index, (left, right) in enumerate(zip(actual, expected)):
        if left != right:
            return "line {}: rust={!r} expected={!r}".format(index + 1, left[:200], right[:200])
    return "line count: rust={} expected={}".format(len(actual), len(expected))


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--filter", default="", help="only run examples whose name contains this")
    parser.add_argument("--verbose", action="store_true", help="print the first diff per failure")
    parser.add_argument("--build", action="store_true", help="run cargo build -p rust-javac first")
    args = parser.parse_args()

    examples_dir = root / "internal" / "test" / "diag-corpus" / "examples"
    expected_dir = root / "internal" / "test" / "diag-corpus" / "expected"
    suffix = ".exe" if os.name == "nt" else ""
    exe = root / "target" / "debug" / ("ravac" + suffix)

    if args.build:
        code = subprocess.call(["cargo", "build", "-p", "rust-javac"], cwd=str(root))
        if code != 0:
            print("cargo build failed", file=sys.stderr)
            return 2
    if not exe.is_file():
        print("rust-javac not found: {} (run with --build)".format(exe), file=sys.stderr)
        return 2
    if not examples_dir.is_dir() or not expected_dir.is_dir():
        print("fixtures not found: {} / {}".format(examples_dir, expected_dir), file=sys.stderr)
        return 2

    env = os.environ.copy()
    tmp_root = root / "target" / "tmp"
    tmp_root.mkdir(parents=True, exist_ok=True)
    env["TMP"] = str(tmp_root)
    env["TEMP"] = str(tmp_root)
    env["TMPDIR"] = str(tmp_root)
    # Keep the per-example directories inside the repository as well: on some
    # Windows hosts a freshly built compiler cannot create files under the
    # user's %TEMP% (access denied), while writing under target/tmp works.
    tempfile.tempdir = str(tmp_root)

    entries = sorted(examples_dir.iterdir(), key=lambda item: item.name)
    total = 0
    passed = 0
    failed = []
    skipped = []
    for entry in entries:
        if not entry.is_dir() and not entry.name.endswith(".java"):
            continue
        name = entry.name[: -len(".java")] if entry.name.endswith(".java") else entry.name
        if args.filter and args.filter not in name:
            continue
        example = Example(name, entry)
        reason = skip_reason(example)
        if reason is not None:
            skipped.append((name, reason))
            continue
        total += 1
        expected_name = name + (".dir.txt" if entry.is_dir() else ".txt")
        expected_path = expected_dir / expected_name
        if not expected_path.is_file():
            failed.append((name, "missing expected file: " + expected_name))
            continue
        temp = Path(tempfile.mkdtemp(prefix="diag-examples-"))
        try:
            actual_text = run_example(example, temp, str(exe), examples_dir, env)
        finally:
            shutil.rmtree(temp, ignore_errors=True)
        actual = [line.rstrip() for line in normalize(actual_text, examples_dir, temp).split("\n")]
        expected = [line.rstrip() for line in expected_path.read_text(encoding="utf-8").split("\n")]
        if actual == expected:
            passed += 1
        else:
            failed.append((name, first_diff(actual, expected)))

    print("DIAG_EXAMPLES total={} pass={} fail={} skip={}".format(total, passed, len(failed), len(skipped)))
    if skipped:
        counts = {}
        for _, reason in skipped:
            counts[reason] = counts.get(reason, 0) + 1
        print("DIAG_EXAMPLES skip reasons: " + ", ".join(
            "{}={}".format(key, counts[key]) for key in sorted(counts)
        ))
    for name, detail in failed:
        print("DIAG_EXAMPLES FAIL {}".format(name))
        if args.verbose:
            print("  " + detail)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
