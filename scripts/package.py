#!/usr/bin/env python3
"""Package ravac as self-contained zips (binary + modules + ct.sym).

    python scripts/package.py --help
    python scripts/package.py --build --release --platforms all
    python scripts/package.py --release --platforms windows-amd64,linux-amd64
    python scripts/package.py --platforms all --format dir

One zip per platform.  A zip contains the platform binary plus the platform
data the compiler reads at run time (modules, ct.sym), so the extracted
directory needs no JDK.  The JDK is used at build time only.

Supported targets: Windows x64, Linux x64/arm64 (static musl) and Android
arm64.  Only targets that are built and verified here are listed; macOS is
not built on Windows and is not declared.  Cross linking uses Rust's
rust-lld for Linux and the Android NDK for Android.
"""

import argparse
import os
import platform
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION = "17.0.19"

# platform tag -> (rust target triple, executable name)
TARGETS = {
    "windows-amd64": ("x86_64-pc-windows-gnu", "ravac.exe"),
    "linux-amd64": ("x86_64-unknown-linux-musl", "ravac"),
    "linux-arm64": ("aarch64-unknown-linux-musl", "ravac"),
    "android-arm64": ("aarch64-linux-android", "ravac"),
}
ALL = list(TARGETS)


def die(message: str) -> int:
    print(message, file=sys.stderr)
    return 2


def host_platform() -> "str | None":
    """This machine's platform tag, or None when it is not supported."""
    machine = platform.machine().lower()
    if os.name == "nt":
        return "windows-amd64" if machine in ("amd64", "x86_64") else None
    return "linux-arm64" if machine in ("amd64", "x86_64") else None


def find_android_linker(triple: str) -> "Path | None":
    """NDK clang for an Android target (highest installed NDK)."""
    prefix = "aarch64-linux-android21-clang" if "aarch64" in triple else "x86_64-linux-android21-clang"
    prebuilt = "windows-x86_64" if os.name == "nt" else "linux-x86_64"
    suffix = ".cmd" if os.name == "nt" else ""

    versions = []
    if os.environ.get("ANDROID_NDK_HOME"):
        versions.append(Path(os.environ["ANDROID_NDK_HOME"]))
    if os.environ.get("ANDROID_HOME"):
        versions += _ndk_versions(Path(os.environ["ANDROID_HOME"]) / "ndk")
    if os.environ.get("LOCALAPPDATA"):
        versions += _ndk_versions(Path(os.environ["LOCALAPPDATA"]) / "Android" / "Sdk" / "ndk")
    for ndk in versions:
        clang = ndk / "toolchains" / "llvm" / "prebuilt" / prebuilt / "bin" / (prefix + suffix)
        if clang.is_file():
            return clang
    return None


def _ndk_versions(root: "Path") -> list:
    if not root.is_dir():
        return []
    return sorted((path for path in root.iterdir() if path.is_dir()), reverse=True)


def find_java_home(explicit: "str | None") -> "Path | None":
    if explicit:
        return Path(explicit)
    if os.environ.get("JAVA_HOME"):
        home = Path(os.environ["JAVA_HOME"])
        if (home / "lib" / "modules").is_file():
            return home
    javac = shutil.which("javac")
    if javac:
        home = Path(javac).resolve().parent.parent
        if (home / "lib" / "modules").is_file():
            return home
    return None


def selected_platforms(value: str) -> list:
    if value == "all":
        return list(ALL)
    chosen = [item.strip() for item in value.split(",") if item.strip()]
    unknown = [item for item in chosen if item not in TARGETS]
    if unknown:
        raise SystemExit("unknown platform(s): {} (choose from {})".format(
            ", ".join(unknown), ", ".join(TARGETS)))
    return chosen


def cross_linker(tag: str, triple: str, explicit: "str | None") -> "str | None":
    """The linker for a cross target, or None to let cargo decide."""
    if explicit:
        return explicit
    if tag.startswith("linux"):
        return "rust-lld"
    if tag.startswith("android"):
        ndk = find_android_linker(triple)
        return str(ndk) if ndk is not None else None
    return None


def build(tag: str, triple: str, release: bool, linker: "str | None") -> bool:
    host = tag == host_platform()
    if not host:
        chosen = cross_linker(tag, triple, linker)
        if chosen:
            key = "CARGO_TARGET_{}_LINKER".format(triple.upper().replace("-", "_"))
            os.environ[key] = chosen
    command = ["cargo", "build", "-p", "rust-javac"]
    if release:
        command.append("--release")
    if not host:
        command += ["--target", triple]
    print("build {}: {}".format(tag, " ".join(command)))
    return subprocess.call(command, cwd=str(ROOT)) == 0


def stage(tag: str, triple: str, exe_name: str, profile: str, data, out_root: Path,
          name: str, fmt: str) -> int:
    host = tag == host_platform()
    exe = ROOT / "target" / ("" if host else triple) / profile / exe_name
    if not exe.is_file():
        return die("binary not found: {} (build it first or install the target toolchain)".format(exe))

    stage_dir = out_root / name
    if stage_dir.exists():
        shutil.rmtree(stage_dir)
    stage_dir.mkdir(parents=True)
    shutil.copy2(exe, stage_dir / exe_name)
    for file_name, source in data:
        if source is not None and source.is_file():
            shutil.copy2(source, stage_dir / file_name)
    if fmt in ("dir", "both"):
        print("stage: {}".format(stage_dir))

    if fmt in ("zip", "both"):
        archive = out_root / (name + ".zip")
        if archive.exists():
            archive.unlink()
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
            for path in sorted(stage_dir.rglob("*")):
                if path.is_file():
                    zf.write(path, path.relative_to(stage_dir).as_posix())
        print("zip:   {} ({:.1f} MB)".format(archive, archive.stat().st_size / (1024 * 1024)))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="run cargo build first")
    parser.add_argument("--release", action="store_true", help="use the release profile")
    parser.add_argument("--out", default="dist", help="output directory (default: dist)")
    parser.add_argument("--java-home", help="JDK home for modules/ct.sym")
    parser.add_argument("--platforms", help="comma list or 'all' (default: this host)")
    parser.add_argument("--name", help="base name (single platform only)")
    parser.add_argument("--linker", help="linker for cross targets (e.g. NDK clang)")
    parser.add_argument("--format", choices=["zip", "dir", "both"], default="both",
                        help="what to produce (default: both)")
    args = parser.parse_args()

    if args.platforms:
        platforms = selected_platforms(args.platforms)
    elif host_platform():
        platforms = [host_platform()]
    else:
        return die("this host is not supported; pass --platforms explicitly")
    if args.name and len(platforms) != 1:
        return die("--name is only valid for a single platform")

    java_home = find_java_home(args.java_home)
    if java_home is None or not (java_home / "lib" / "modules").is_file():
        return die("modules not found; set JAVA_HOME or --java-home")
    data = [
        ("modules", java_home / "lib" / "modules"),
        ("ct.sym", java_home / "lib" / "ct.sym"),
    ]
    profile = "release" if args.release else "debug"
    out_root = ROOT / args.out

    for tag in platforms:
        triple, exe_name = TARGETS[tag]
        if args.build and not build(tag, triple, args.release, args.linker):
            return die("cargo build failed for {}".format(tag))
        name = args.name or "ravac-{}-{}".format(VERSION, tag)
        if stage(tag, triple, exe_name, profile, data, out_root, name, args.format) != 0:
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
