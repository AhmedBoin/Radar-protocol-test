#!/usr/bin/env python3
"""Show the app<->radar conversation by direction and message type."""
import collections

P = r"D:\Development\fdad_radar\capture\samples\radar-live-sample.tsv"
dirs = collections.Counter()
types = collections.defaultdict(collections.Counter)
first = {}

for line in open(P, encoding="utf-8", errors="replace"):
    p = line.rstrip("\n").split("\t")
    if len(p) < 5:
        continue
    d, api, hx = p[1], p[2], (p[4] or "").strip()
    dirs[(d, api)] += 1
    if not hx:
        continue
    try:
        b = bytes.fromhex(hx)
    except ValueError:
        continue
    if b[:4] != b"\x55\xaa\x55\xaa":
        continue
    L = int.from_bytes(b[4:8], "big")
    if 8 + L > len(b) or L < 2:
        continue
    pl = b[8:8 + L - 2]
    if len(pl) < 4:
        continue
    t = int.from_bytes(pl[:4], "big")
    types[d][t] += 1
    if (d, t) not in first:
        first[(d, t)] = hx

print("=== DIR/API counts ===")
for k, v in dirs.most_common():
    print(" ", k, v)

print("\n=== message types by direction ===")
for d, c in types.items():
    print(" ", d, {hex(k): v for k, v in c.most_common()})

print("\n=== first example of each (dir,type) ===")
for (d, t), v in sorted(first.items(), key=lambda x: (x[0][0], x[0][1])):
    print("\n  dir=%s type=0x%08X len=%d" % (d, t, len(v) // 2))
    print("   ", v[:160])
