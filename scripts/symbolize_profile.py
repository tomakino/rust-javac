#!/usr/bin/env python3
"""Aggregate a samply/Firefox profile by resolving raw module addresses.

samply cannot symbolize `x86_64-pc-windows-gnu` binaries (it looks for PDB,
the GNU toolchain emits DWARF), so the profile only carries module-relative
addresses. `llvm-nm` does expose the symbol table of the same executable, so a
nearest-symbol lookup recovers function names without a full DWARF reader.

Usage:
    python scripts/symbolize_profile.py --profile target/tmp/jb-profile2.json \
        --nm target/tmp/nm.txt --base 0x140000000 --top 25
"""

import argparse
import bisect
import json
import re
import sys
from collections import Counter

SYMBOL = re.compile(r"^([0-9a-fA-F]+)\s+[TtWw]\s+(.+)$")
ADDRESS = re.compile(r"^0x([0-9a-fA-F]+)$")


def load_symbols(path, image_base):
    entries = []
    with open(path, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            match = SYMBOL.match(line.rstrip("\n"))
            if not match:
                continue
            address = int(match.group(1), 16)
            if address < image_base:
                continue
            entries.append((address - image_base, match.group(2).strip()))
    entries.sort()
    return entries


def resolve(entries, rva):
    index = bisect.bisect_right(entries, (rva, "\uffff")) - 1
    if index < 0:
        return None
    address, name = entries[index]
    return name


def parse_address(text):
    match = ADDRESS.match(text.strip())
    return int(match.group(1), 16) if match else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--nm", required=True)
    parser.add_argument("--base", default="0x140000000")
    parser.add_argument("--top", type=int, default=25)
    parser.add_argument("--thread", default="")
    parser.add_argument(
        "--lib",
        default="ravac.exe",
        help="only count samples whose leaf frame belongs to this library",
    )
    args = parser.parse_args()

    base = int(args.base, 16)
    entries = load_symbols(args.nm, base)
    if not entries:
        print("no symbols parsed from {}".format(args.nm), file=sys.stderr)
        return 2

    with open(args.profile, encoding="utf-8") as handle:
        profile = json.load(handle)

    libraries = profile.get("libs") or []
    threads = profile.get("threads") or []
    totals = 0
    per_thread = []
    for thread in threads:
        name = thread.get("name") or ""
        if args.thread and args.thread.lower() not in name.lower():
            continue
        strings = thread.get("stringArray") or []
        funcs = thread.get("funcTable") or {}
        frames = thread.get("frameTable") or {}
        stacks = thread.get("stackTable") or {}
        samples = thread.get("samples") or {}
        func_names = funcs.get("name") or []
        func_resources = funcs.get("resource") or []
        frame_funcs = frames.get("func") or []
        stack_frames = stacks.get("frame") or []
        stack_ids = samples.get("stack") or []
        weights = samples.get("weight") or []
        resource_libs = (thread.get("resourceTable") or {}).get("lib") or []

        counts = Counter()
        thread_total = 0
        for index, stack_id in enumerate(stack_ids):
            weight = weights[index] if index < len(weights) else 1
            if stack_id is None or stack_id >= len(stack_frames):
                continue
            frame_id = stack_frames[stack_id]
            if frame_id is None or frame_id >= len(frame_funcs):
                continue
            func_id = frame_funcs[frame_id]
            if func_id is None or func_id >= len(func_names):
                continue
            resource_id = (
                func_resources[func_id] if func_id < len(func_resources) else None
            )
            library_name = ""
            if resource_id is not None and resource_id < len(resource_libs):
                lib_id = resource_libs[resource_id]
                if lib_id is not None and lib_id < len(libraries):
                    library_name = (
                        libraries[lib_id].get("name")
                        or libraries[lib_id].get("debugName")
                        or ""
                    )
            if args.lib and args.lib not in library_name:
                continue
            thread_total += weight
            name_id = func_names[func_id]
            if name_id is None or name_id >= len(strings):
                continue
            rva = parse_address(strings[name_id])
            if rva is None:
                continue
            symbol = resolve(entries, rva)
            if symbol is None:
                continue
            counts[symbol] += weight

        totals += thread_total
        per_thread.append((name, thread_total, counts))

    if not totals:
        print("no samples resolved", file=sys.stderr)
        return 1

    merged = Counter()
    for _, _, counts in per_thread:
        merged.update(counts)

    print("total samples: {}  symbols: {}".format(totals, len(entries)))
    print()
    print("{:>10}  {:>6}  {}".format("samples", "pct", "function"))
    for symbol, count in merged.most_common(args.top):
        print("{:>10}  {:>5.1f}%  {}".format(count, count * 100.0 / totals, symbol))
    return 0


if __name__ == "__main__":
    sys.exit(main())
