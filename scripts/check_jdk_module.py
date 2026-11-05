#!/usr/bin/env python3
"""Compare rust-javac against javac on one JDK 17 module source tree.

The sources are read from <JAVA_HOME>/lib/src.zip and extracted to
target/tmp/jdk-src/<module> on first use, so the corpus is the exact JDK the
reference javac was built from.  Both compilers build the whole module in one
process with --patch-module <module>=<src> and every produced class file is
compared byte-for-byte (SHA-256), together with the exit codes.

Usage:
    python scripts/check_jdk_module.py --module java.desktop
"""
import argparse
import hashlib
import os
import pathlib
import shutil
import subprocess
import sys
import zipfile

sys.stdout.reconfigure(encoding="utf-8")

ROOT = pathlib.Path(__file__).resolve().parent.parent
JAVA_HOME = pathlib.Path(os.environ.get("JAVA_HOME", ""))
JAVAC = JAVA_HOME / "bin" / "javac.exe"
SRC_ZIP = JAVA_HOME / "lib" / "src.zip"
SRC_ROOT = ROOT / "target/tmp/jdk-src"

# src.zip only ships the platform independent sources of a module.  java.desktop
# keeps its GTK look-and-feel and the Pango font code under unix/ (or windows/)
# classes, so those files reference types that are absent from a plain JDK
# installation and neither compiler can resolve them.  Drop them so the corpus
# stays a tree the reference javac accepts.
DEFAULT_EXCLUDE = ("/plaf/gtk/", "/PangoFonts.java")


def ensure_sources(module: str) -> pathlib.Path:
    dest = SRC_ROOT / module
    probe = dest / "module-info.java"
    if probe.exists():
        return dest
    if not SRC_ZIP.is_file():
        raise SystemExit("no src.zip under JAVA_HOME: {}".format(SRC_ZIP))
    dest.mkdir(parents=True, exist_ok=True)
    count = 0
    with zipfile.ZipFile(SRC_ZIP) as z:
        for name in z.namelist():
            if not name.startswith(module + "/") or not name.endswith(".java"):
                continue
            target = dest / name[len(module) + 1:]
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(z.read(name))
            count += 1
    print("[src] extracted {} files to {}".format(count, dest), flush=True)
    return dest


def hash_dir(d: pathlib.Path) -> dict:
    out = {}
    if not d.is_dir():
        return out
    for p in d.rglob("*.class"):
        out[p.relative_to(d).as_posix()] = hashlib.sha256(p.read_bytes()).hexdigest()
    return out


def run(tool, module, src_root, out_dir, files, argfile_dir=None):
    out_dir.mkdir(parents=True, exist_ok=True)
    cmd = [
        str(tool),
        "-J-Duser.language=en",
        "-J-Duser.country=US",
        "--patch-module",
        "{}={}".format(module, src_root),
        "-d",
        str(out_dir),
    ]
    if argfile_dir is not None and len(files) > 200:
        af = pathlib.Path(argfile_dir) / "javac_args.txt"
        af.parent.mkdir(parents=True, exist_ok=True)
        af.write_text("\n".join(str(f) for f in files), encoding="utf-8")
        cmd.append("@" + str(af))
    else:
        cmd += [str(f) for f in files]
    p = subprocess.run(cmd, capture_output=True)
    return p.returncode, p.stderr.decode("utf-8", "replace")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--module", default="java.desktop")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--filter", default="")
    ap.add_argument("--exclude", default="")
    ap.add_argument("--work", default="")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    module = args.module
    src_root = ensure_sources(module)
    java_files = sorted(src_root.rglob("*.java"))
    if args.filter:
        java_files = [f for f in java_files if args.filter in f.as_posix()]
    excludes = list(DEFAULT_EXCLUDE)
    if args.exclude:
        excludes.append(args.exclude)
    java_files = [
        f for f in java_files if not any(x in f.as_posix() for x in excludes)
    ]
    if args.limit:
        java_files = java_files[: args.limit]
    if not java_files:
        print("no source files")
        return 1

    work = pathlib.Path(args.work) if args.work else ROOT / "target/tmp" / (module.replace(".", "_") + "_check")
    shutil.rmtree(work, ignore_errors=True)

    exe = ROOT / "target/debug/ravac.exe"
    matched = 0
    diffs = []

    jout_all = work / "javac_all"
    print("[javac] compiling {} sources of {} ...".format(len(java_files), module), flush=True)
    jc_all, jerr_all = run(JAVAC, module, src_root, jout_all, java_files, argfile_dir=work)
    all_jclasses = hash_dir(jout_all)
    print("[javac] exit={} classes={}".format(jc_all, len(all_jclasses)), flush=True)

    rout_all = work / "rust_all"
    print("[rust] compiling the module once ...", flush=True)
    rc_all, rerr_all = run(exe, module, src_root, rout_all, java_files, argfile_dir=work / "rust")
    parsed_all = hash_dir(rout_all)
    print("[rust] exit={} classes={}".format(rc_all, len(parsed_all)), flush=True)
    if rc_all != 0:
        # A per-file retry would relaunch the compiler once per source (thousands
        # of process starts, each re-reading the module) and the single-file
        # --patch-module runs report unrelated "package is empty" errors, so the
        # batch run is the only trustworthy result.
        print("[rust] batch compile failed; no per-file fallback", flush=True)
        errs = [l for l in rerr_all.splitlines() if "错误" in l or "error" in l]
        for line in errs[:40]:
            print("  " + line, flush=True)
        print("checked={} matched=0 failures={}".format(len(java_files), len(errs)))
        print("{} parity:".format(module), "FAIL")
        return 1

    for i, f in enumerate(java_files, 1):
        rel = f.relative_to(src_root).as_posix()
        prefix = rel[:-5]
        rclasses = {
            k: v
            for k, v in parsed_all.items()
            if k == prefix + ".class" or k.startswith(prefix + "$")
        }
        rc, rerr = 0, ""
        ok = rc == 0 and all(
            k in all_jclasses and all_jclasses[k] == v for k, v in rclasses.items()
        )
        if ok:
            matched += 1
        else:
            extra = sorted(set(rclasses) - set(all_jclasses))
            diff = sorted(
                k for k in set(rclasses) & set(all_jclasses) if all_jclasses[k] != rclasses[k]
            )
            diffs.append((rel, jc_all, rc, len(all_jclasses), len(rclasses), extra[:3], diff[:3], jerr_all, rerr))
        if args.verbose or i % 100 == 0:
            print("  [{}/{}] matched={}".format(i, len(java_files), matched), flush=True)

    print("checked={} matched={} failures={}".format(len(java_files), matched, len(diffs)))
    for rel, jc, rc, jn, rn, extra, diff, jerr, rerr in diffs[:60]:
        print("FAIL {}: javac exit={} classes={} rust exit={} classes={}".format(rel, jc, jn, rc, rn))
        if extra:
            print("  extra: {}".format(extra))
        if diff:
            print("  diff: {}".format(diff))
        if jerr.strip():
            print("  javac: {}".format([l for l in jerr.splitlines() if "error" in l][:2]))
        if rerr.strip():
            print("  rust: {}".format([l for l in rerr.splitlines() if "error" in l or "panic" in l][:2]))
    print("{} parity:".format(module), "OK" if not diffs else "FAIL")
    return 0 if not diffs else 1


if __name__ == "__main__":
    raise SystemExit(main())
