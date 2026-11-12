#!/usr/bin/env python3
"""Aggregate a samply profile by DWARF line, not by symbol name.

samply records raw module-relative addresses for `x86_64-pc-windows-gnu`
binaries and does not fill `frameTable.line`. Nearest-symbol lookup is too
coarse for Rust: inlined and generic code blurs function boundaries, so two
rebuilds can attribute the same samples to different functions.

`llvm-objdump --dwarf=decodedline` gives the address-to-source-line map, which
survives inlining (every inlined instruction keeps its own line).

Usage:
    python scripts/symbolize_lines.py --profile target/tmp/jb-profile3.json \
        --lines target/tmp/lines.txt --base 0x140000000 --top 30
"""

import argparse
import bisect
import json
import re
import sys
from collections import Counter

LINE = re.compile(r"^(\S+)\s+(\d+)\s+(0x[0-9a-fA-F]+)")
ADDRESS = re.compile(r"^0x([0-9a-fA-F]+)$")


def load_lines(path):
    addresses = []
    locations = []
    with open(path, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if not line or line[0].isspace():
                continue
            match = LINE.match(line)
            if not match:
                continue
            addresses.append(int(match.group(3), 16))
            locations.append("{}:{}".format(match.group(1), match.group(2)))
    if not addresses:
        return [], []
    order = sorted(range(len(addresses)), key=addresses.__getitem__)
    return [addresses[i] for i in order], [locations[i] for i in order]


def resolve(addresses, locations, address):
    index = bisect.bisect_right(addresses, address) - 1
    if index < 0:
        return None
    return locations[index]


def parse_address(text):
    match = ADDRESS.match(text.strip())
    return int(match.group(1), 16) if match else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--lines", required=True)
    parser.add_argument("--base", default="0x140000000")
    parser.add_argument("--top", type=int, default=30)
    args = parser.parse_args()

    base = int(args.base, 16)
    addresses, locations = load_lines(args.lines)
    if not addresses:
        print("no line records parsed from {}".format(args.lines), file=sys.stderr)
        return 2

    with open(args.profile, encoding="utf-8") as handle:
        profile = json.load(handle)

    totals = 0
    merged = Counter()
    unresolved = 0
    for thread in profile.get("threads") or []:
        strings = thread.get("stringArray") or []
        funcs = thread.get("funcTable") or {}
        frames = thread.get("frameTable") or {}
        stacks = thread.get("stackTable") or {}
        samples = thread.get("samples") or {}
        func_names = funcs.get("name") or []
        frame_funcs = frames.get("func") or []
        stack_frames = stacks.get("frame") or []
        stack_ids = samples.get("stack") or []
        weights = samples.get("weight") or []

        for index, stack_id in enumerate(stack_ids):
            weight = weights[index] if index < len(weights) else 1
            totals += weight
            if stack_id is None or stack_id >= len(stack_frames):
                continue
            frame_id = stack_frames[stack_id]
            if frame_id is None or frame_id >= len(frame_funcs):
                continue
            func_id = frame_funcs[frame_id]
            if func_id is None or func_id >= len(func_names):
                continue
            name_id = func_names[func_id]
            if name_id is None or name_id >= len(strings):
                continue
            rva = parse_address(strings[name_id])
            if rva is None:
                continue
            location = resolve(addresses, locations, rva + base)
            if location is None:
                unresolved += weight
                continue
            merged[location] += weight

    if not totals:
        print("no samples", file=sys.stderr)
        return 1

    print("total samples: {}  line records: {}  unresolved: {}".format(
        totals, len(addresses), unresolved))
    print()
    print("{:>10}  {:>6}  {}".format("samples", "pct", "source line"))
    for location, count in merged.most_common(args.top):
        print("{:>10}  {:>5.1f}%  {}".format(count, count * 100.0 / totals, location))
    return 0


if __name__ == "__main__":
    sys.exit(main())
