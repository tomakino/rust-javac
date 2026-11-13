#!/usr/bin/env python3
"""Package ravac as self-contained binaries.

    python scripts/package.py --help
    python scripts/package.py --build --release --platforms all
    python scripts/package.py --release --platforms windows-amd64,linux-amd64
    python scripts/package.py --platforms all --format dir

One zip per platform, containing only the executable: the platform image
(modules) and ct.sym are compiled into the binary, so the extracted file needs
no JDK and no data files.

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

# The NDK API level the Android binary targets.
ANDROID_API = "21"


def die(message: str) -> int:
    print(message, file=sys.stderr)
    return 2


def host_platform() -> "str | None":
    """This machine's platform tag, or None when it is not supported."""
    machine = platform.machine().lower()
    if os.name == "nt":
        return "windows-amd64" if machine in ("amd64", "x86_64") else None
    return "linux-arm64" if machine in ("amd64", "x86_64") else None


def _ndk_versions(root: Path) -> list:
    if not root.is_dir():
        return []
    return sorted((path for path in root.iterdir() if path.is_dir()), reverse=True)


def find_android_toolchain() -> "Path | None":
    """The NDK clang bin directory (highest installed NDK), or None."""
    prebuilt = "windows-x86_64" if os.name == "nt" else "linux-x86_64"
    roots = []
    if os.environ.get("ANDROID_NDK_HOME"):
        roots.append(Path(os.environ["ANDROID_NDK_HOME"]))
    if os.environ.get("ANDROID_HOME"):
        roots += _ndk_versions(Path(os.environ["ANDROID_HOME"]) / "ndk")
    if os.environ.get("LOCALAPPDATA"):
        roots += _ndk_versions(Path(os.environ["LOCALAPPDATA"]) / "Android" / "Sdk" / "ndk")
    for ndk in roots:
        bin_dir = ndk / "toolchains" / "llvm" / "prebuilt" / prebuilt / "bin"
        if (bin_dir / "clang.exe").is_file() or (bin_dir / "clang").is_file():
            return bin_dir
    return None


def find_mingw_toolchain() -> "Path | None":
    """The LLVM-MinGW bin directory installed by winget, or None.

    mimalloc is C, so the Windows host build needs a MinGW C compiler; the
    AGENTS.md setup installs LLVM-MinGW through winget.
    """
    if os.name != "nt":
        return None
    local = os.environ.get("LOCALAPPDATA")
    if not local:
        return None
    packages = Path(local) / "Microsoft" / "WinGet" / "Packages"
    if not packages.is_dir():
        return None
    roots = []
    for entry in packages.glob("MartinStorsjo.LLVM-MinGW.UCRT_*"):
        roots += sorted(entry.glob("llvm-mingw-*-ucrt-x86_64"), reverse=True)
    for root in roots:
        bin_dir = root / "bin"
        if (bin_dir / "clang.exe").is_file():
            return bin_dir
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


def cross_env(tag: str, triple: str, linker: "str | None") -> dict:
    """Environment for a cross build (empty for the host platform)."""
    if tag == host_platform():
        return {}
    env = {}
    linker_key = "CARGO_TARGET_{}_LINKER".format(triple.upper().replace("-", "_"))
    if tag.startswith("linux"):
        env[linker_key] = "rust-lld"
        return env
    if tag.startswith("android"):
        bin_dir = find_android_toolchain()
        if bin_dir is None:
            return env
        suffix = ".exe" if os.name == "nt" else ""
        clang = str(bin_dir / ("clang" + suffix))
        target = triple + ANDROID_API
        key = triple.replace("-", "_")
        env["CC_" + key] = clang
        env["AR_" + key] = str(bin_dir / ("llvm-ar" + suffix))
        env["CFLAGS_" + key] = "--target={} -mno-outline-atomics".format(target)
        # The clang driver must learn the target explicitly: as a bare linker it
        # receives GNU-ld flags and lld rejects them ("unknown argument:
        # --eh-frame-hdr").
        env["CCC_OVERRIDE_OPTIONS"] = "+--target=" + target
        env[linker_key] = clang
        # Without this the C side calls __aarch64_ldset8_* helpers that the
        # NDK's compiler-rt does not define and the link fails.
        env["RUSTFLAGS_" + key] = "-C target-feature=-outline-atomics"
        if linker:
            env[linker_key] = linker
    return env


def host_env(tag: str, triple: str) -> dict:
    """Environment for the host build (the C compiler mimalloc needs)."""
    if tag != "windows-amd64":
        return {}
    key = triple.replace("-", "_")
    if os.environ.get("CC_" + key):
        return {}
    bin_dir = find_mingw_toolchain()
    if bin_dir is None:
        return {}
    return {
        "CC_" + key: str(bin_dir / "clang.exe"),
        "AR_" + key: str(bin_dir / "llvm-ar.exe"),
    }


def build(tag: str, triple: str, release: bool, linker: "str | None") -> bool:
    host = tag == host_platform()
    env = dict(os.environ)
    if host:
        env.update(host_env(tag, triple))
    else:
        env.update(cross_env(tag, triple, linker))
    command = ["cargo", "build", "-p", "rust-javac"]
    if release:
        command.append("--release")
    if not host:
        command += ["--target", triple]
    print("build {}: {}".format(tag, " ".join(command)))
    return subprocess.call(command, cwd=str(ROOT), env=env) == 0


def stage(tag: str, triple: str, exe_name: str, profile: str, out_root: Path,
          name: str, fmt: str) -> int:
    host = tag == host_platform()
    exe = ROOT / "target" / ("" if host else triple) / profile / exe_name
    if not exe.is_file():
        return die("binary not found: {} (build it first or install the target toolchain)".format(exe))

    stage_dir = out_root / name
    if stage_dir.exists():
        shutil.rmtree(stage_dir)
    stage_dir.mkdir(parents=True)
    # The binary embeds modules and ct.sym; nothing external is needed.
    shutil.copy2(exe, stage_dir / exe_name)
    # The shared library (cdylib) ships on the platforms where the C ABI is
    # meant to be used (Android/Linux); the Windows package is CLI only.
    if not exe_name.endswith(".exe"):
        shared = exe.with_name("libravac.so")
        if shared.is_file():
            shutil.copy2(shared, stage_dir / shared.name)
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

    profile = "release" if args.release else "debug"
    out_root = ROOT / args.out

    for tag in platforms:
        triple, exe_name = TARGETS[tag]
        if args.build and not build(tag, triple, args.release, args.linker):
            return die("cargo build failed for {}".format(tag))
        name = args.name or "ravac-{}-{}".format(VERSION, tag)
        if stage(tag, triple, exe_name, profile, out_root, name, args.format) != 0:
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
