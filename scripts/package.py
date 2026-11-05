#!/usr/bin/env python3
"""Build a self-contained ravac distribution (zip).

The archive contains ravac.exe plus the JDK platform data it needs at run
time, so the extracted directory works without a JDK installation:

    ravac.exe
    modules         (jimage: system classes for every module)
    ct.sym          (--release signatures)

Usage:
    python scripts/package.py --build [--release] [--out dist]

Exit codes: 0 success, 2 setup missing.
"""

import argparse
import os
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION = "17.0.19"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", action="store_true", help="use cargo build --release")
    parser.add_argument("--build", action="store_true", help="run cargo build first")
    parser.add_argument("--out", default="dist", help="output directory (default: dist)")
    args = parser.parse_args()

    java_home = Path(os.environ.get("JAVA_HOME", ""))
    modules = java_home / "lib" / "modules"
    ct_sym = java_home / "lib" / "ct.sym"
    if not java_home.is_dir() or not modules.is_file():
        print("JAVA_HOME with lib/modules is required: {}".format(java_home), file=sys.stderr)
        return 2

    if args.build:
        command = ["cargo", "build", "-p", "rust-javac"]
        if args.release:
            command.append("--release")
        if subprocess.call(command, cwd=str(ROOT)) != 0:
            print("cargo build failed", file=sys.stderr)
            return 2

    profile = "release" if args.release else "debug"
    exe_name = "ravac.exe" if os.name == "nt" else "ravac"
    exe = ROOT / "target" / profile / exe_name
    if not exe.is_file():
        print("binary not found: {}".format(exe), file=sys.stderr)
        return 2

    out = ROOT / args.out
    stage = out / "ravac-win64"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    shutil.copy2(exe, stage / exe_name)
    shutil.copy2(modules, stage / "modules")
    if ct_sym.is_file():
        shutil.copy2(ct_sym, stage / "ct.sym")

    archive = out / "ravac-{}-win64.zip".format(VERSION)
    if archive.exists():
        archive.unlink()
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for path in sorted(stage.rglob("*")):
            if path.is_file():
                zf.write(path, path.relative_to(stage).as_posix())

    print("stage: {}".format(stage))
    print("dist:  {}".format(archive))
    print("size:  {:.1f} MB".format(archive.stat().st_size / (1024 * 1024)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
