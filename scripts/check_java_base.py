#!/usr/bin/env python3
"""Compare rust-javac against javac on the JDK 17 java.base source tree.

Compiles each java.base source file with --patch-module java.base=<src> and
compares the produced class files byte-for-byte plus the diagnostics/exit code.
"""
import argparse
import hashlib
import os
import pathlib
import shutil
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8")

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_SRC = ROOT / "target/tmp/jdk-src/java.base"
JAVAC = pathlib.Path(os.environ.get("JAVA_HOME", "")) / "bin/javac.exe"


def hash_dir(d):
    out = {}
    if not d.exists():
        return out
    for p in d.rglob("*.class"):
        out[p.relative_to(d).as_posix()] = hashlib.sha256(p.read_bytes()).hexdigest()
    return out


def run(tool, src_root, out_dir, files, argfile_dir=None):
    out_dir.mkdir(parents=True, exist_ok=True)
    cmd = [str(tool), "-J-Duser.language=en", "-J-Duser.country=US",
           "--patch-module", "java.base=" + str(src_root),
           "-d", str(out_dir)]
    if argfile_dir is not None and len(files) > 200:
        # Windows caps the command line at 32767 characters; javac accepts
        # "@file" for long argument lists.
        af = pathlib.Path(argfile_dir) / "javac_args.txt"
        af.parent.mkdir(parents=True, exist_ok=True)
        af.write_text("\n".join(str(f) for f in files), encoding="utf-8")
        cmd.append("@" + str(af))
    else:
        cmd += [str(f) for f in files]
    p = subprocess.run(cmd, capture_output=True)
    err = p.stderr.decode("utf-8", "replace")
    return p.returncode, err


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", default=str(DEFAULT_SRC))
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--filter", default="")
    ap.add_argument("--exclude", default="")
    ap.add_argument("--work", default=str(ROOT / "target/tmp/java_base_check"))
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    src_root = pathlib.Path(args.src)
    java_files = sorted(src_root.rglob("*.java"))
    if args.filter:
        java_files = [f for f in java_files if args.filter in f.as_posix()]
    if args.exclude:
        # A source that crashes the compiler would abort the whole batch, so the
        # known-offender can be dropped while the rest is compared in one run.
        java_files = [f for f in java_files if args.exclude not in f.as_posix()]
    if args.limit:
        java_files = java_files[: args.limit]
    if not java_files:
        print("no source files")
        return 1

    work = pathlib.Path(args.work)
    shutil.rmtree(work, ignore_errors=True)

    exe = ROOT / "target/debug/ravac.exe"
    matched = 0
    diffs = []
    # javac's --patch-module always compiles the whole module, so run it once
    # over the complete source list and compare each rust output against the
    # matching class from that single run.
    jout_all = work / "javac_all"
    print("[javac] compiling the module once...", flush=True)
    jc_all, jerr_all = run(JAVAC, src_root, jout_all, java_files, argfile_dir=work)
    all_jclasses = hash_dir(jout_all)
    print(f"[javac] exit={jc_all} classes={len(all_jclasses)}", flush=True)
    # Compile the whole module with rust in one process, the same way javac was
    # invoked: spawning one process per source file dominated the runtime
    # (3067 process starts) without adding any isolation that this check needs.
    # On a non-zero exit the per-file path below retries each file so a single
    # failing source still produces a precise per-file report.
    print("[rust] compiling the module once...", flush=True)
    rout_all = work / "rust_all"
    rc_all, rerr_all = run(exe, src_root, rout_all, java_files, argfile_dir=work / "rust")
    parsed_all = hash_dir(rout_all)
    print(f"[rust] exit={rc_all} classes={len(parsed_all)}", flush=True)
    batch_ok = rc_all == 0
    for i, f in enumerate(java_files, 1):
        rel = f.relative_to(src_root).as_posix()
        prefix = rel[:-5]
        if batch_ok:
            rclasses = {
                k: v
                for k, v in parsed_all.items()
                if k == prefix + ".class" or k.startswith(prefix + "$")
            }
            rc, rerr = 0, ""
        else:
            rout = work / "rust" / rel.replace("/", "_")[:-5]
            rc, rerr = run(exe, src_root, rout, [f])
            rclasses = hash_dir(rout)
        # A package-info.java legitimately produces no class file.
        ok = (
            rc == 0
            and all(k in all_jclasses and all_jclasses[k] == v for k, v in rclasses.items())
        )
        if ok:
            matched += 1
        else:
            extra = sorted(set(rclasses) - set(all_jclasses))
            diff = sorted(
                k for k in set(rclasses) & set(all_jclasses) if all_jclasses[k] != rclasses[k]
            )
            missing = []
            diffs.append((rel, jc_all, rc, len(all_jclasses), len(rclasses), missing[:3], extra[:3], diff[:3], jerr_all, rerr))
        if args.verbose or i % 100 == 0:
            print(f"  [{i}/{len(java_files)}] matched={matched}", flush=True)

    print(f"checked={len(java_files)} matched={matched} failures={len(diffs)}")
    for rel, jc, rc, jn, rn, missing, extra, diff, jerr, rerr in diffs[:60]:
        print(f"FAIL {rel}: javac exit={jc} classes={jn} rust exit={rc} classes={rn}")
        if missing:
            print(f"  missing: {missing}")
        if extra:
            print(f"  extra: {extra}")
        if diff:
            print(f"  diff: {diff}")
        if jerr.strip():
            print(f"  javac: {[l for l in jerr.splitlines() if 'error' in l][:2]}")
        if rerr.strip():
            print(f"  rust: {[l for l in rerr.splitlines() if 'error' in l or 'panic' in l][:2]}")
    print("java.base parity:", "OK" if not diffs else "FAIL")
    return 0 if not diffs else 1


if __name__ == "__main__":
    sys.exit(main())