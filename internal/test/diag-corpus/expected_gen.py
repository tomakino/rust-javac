#!/usr/bin/env python3
"""Generate frozen diagnostic baselines under expected/ by running official javac.

Self-contained: only needs a JDK 17 javac binary, not the upstream
Example/CheckExamples harness. Argument construction mirrors
crates/rust-javac/tests/diag_examples.rs so a rust-javac run can be compared
1:1 against the frozen text.

Usage:
    python ExpectedGen.py <examplesDir> <expectedDir> [javac]

A file example produces <name>.txt; a directory example produces
<name>.dir.txt, matching the Rust harness.
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


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


def run_javac(javac: str, args: list, cwd: Path):
    command = [javac, "-J-Duser.language=en", "-J-Duser.country=US"] + args
    proc = subprocess.run(
        command,
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    stdout = proc.stdout.decode("utf-8", errors="replace")
    stderr = proc.stderr.decode("utf-8", errors="replace")
    return stdout, stderr, proc.returncode == 0


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
    # The Windows javac launcher prints the argument-file path when it aborts
    # with an internal error (exit 4). The line carries a timestamp and the
    # current directory, so it is not part of the compiler diagnostics and
    # must not be frozen into the baselines.
    result = re.sub(r"(?m)^printing javac parameters to: .*\r?\n?", "", result)
    return result


def run_example(example: Example, temp: Path, javac: str, examples_dir: Path) -> str:
    classes = temp / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    args = []
    prefix_errors = ""

    if example.class_path:
        classpath_dir = temp / "classpath"
        classpath_dir.mkdir(parents=True, exist_ok=True)
        cp_args = ["-d", str(classpath_dir)] + [str(item) for item in example.class_path]
        _, cp_err, cp_ok = run_javac(javac, cp_args, temp)
        if not cp_ok:
            prefix_errors += cp_err
        args += ["--class-path", str(classpath_dir)]

    args += ["-d", str(classes)]
    args += example.options

    if example.additional:
        extra = ["-d", str(classes)] + [str(item) for item in example.additional]
        run_javac(javac, extra, temp)

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
        run_javac(javac, module_args, temp)
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

    stdout, stderr, _ = run_javac(javac, args, temp)
    return normalize(stdout + stderr + prefix_errors, examples_dir, temp)


def resolve_javac(argv) -> str:
    if len(argv) > 3:
        return argv[3]
    home = os.environ.get("JAVA_HOME")
    if home:
        candidate = Path(home) / "bin" / ("javac.exe" if os.name == "nt" else "javac")
        if candidate.is_file():
            return str(candidate)
    found = shutil.which("javac")
    return found or ""


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: ExpectedGen.py <examplesDir> <expectedDir> [javac]", file=sys.stderr)
        return 2
    examples_dir = Path(sys.argv[1]).resolve()
    out_dir = Path(sys.argv[2]).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    javac = resolve_javac(sys.argv)
    if not javac or not Path(javac).is_file():
        print("javac not found: " + repr(javac), file=sys.stderr)
        return 2
    if not examples_dir.is_dir():
        print("examples directory not found: " + str(examples_dir), file=sys.stderr)
        return 2

    entries = sorted(examples_dir.iterdir(), key=lambda item: item.name)
    ok = 0
    errors = 0
    for entry in entries:
        if not entry.is_dir() and not entry.name.endswith(".java"):
            continue
        name = entry.name[:-len(".java")] if entry.name.endswith(".java") else entry.name
        temp = Path(tempfile.mkdtemp(prefix="expected-gen-"))
        try:
            text = run_example(Example(name, entry), temp, javac, examples_dir)
            suffix = ".dir.txt" if entry.is_dir() else ".txt"
            (out_dir / (name + suffix)).write_text(text, encoding="utf-8")
            ok += 1
        except Exception as error:  # noqa: BLE001 - report and keep going
            print("ERROR " + name + ": " + str(error), file=sys.stderr)
            errors += 1
        finally:
            shutil.rmtree(temp, ignore_errors=True)

    print("ok=" + str(ok) + " errors=" + str(errors))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
