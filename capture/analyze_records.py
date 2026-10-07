#!/usr/bin/env python3
"""Dump full 64-byte records to figure out the untouched trailing 32 bytes."""
import struct

PATH = r"D:\Development\fdad_radar\capture\samples\radar-live-sample.tsv"

def crc16(d):
    c = 0xFFFF
    for x in d:
        c ^= x
        for _ in range(8):
            c = (c >> 1) ^ 0xA001 if (c & 1) else c >> 1
    return c

shown = 0
with open(PATH, encoding="utf-8", errors="replace") as f:
    for line in f:
        p = line.rstrip("\n").split("\t")
        if len(p) < 5: continue
        hx = (p[4] or "").strip()
        if not hx: continue
        try: b = bytes.fromhex(hx)
        except ValueError: continue
        if b[:4] != b"\x55\xaa\x55\xaa": continue
        L = int.from_bytes(b[4:8], "big")
        if 8 + L > len(b) or L < 2: continue
        pl = b[8:8 + L - 2]
        if crc16(pl) != int.from_bytes(b[8+L-2:8+L], "little"): continue
        if int.from_bytes(pl[:4], "big") != 0x00030002 or len(pl) < 48: continue
        rec = pl[8:][32:]
        n = int.from_bytes(rec[:8], "big")
        if not n or 8 + n * 64 > len(rec): continue
        for k in range(n):
            r = rec[8 + k*64: 8 + k*64 + 64]
            d = struct.unpack(">16i", r)
            print("rec %2d |" % k, " ".join("%08x" % (x & 0xffffffff) for x in d[:8]), "|",
                  " ".join("%08x" % (x & 0xffffffff) for x in d[8:16]))
            print("        X=%d Y=%d H=%d Vx=%d Vy=%d type=%d id=%d pk=%d | tail=%s"
                  % (d[0], d[1], d[2], d[3], d[4], d[5], d[6] & 0xffffffff, d[7] & 0xffffffff, d[8:16]))
            shown += 1
            if shown >= 10: raise SystemExit
