#!/usr/bin/env python3
"""Analyze the real radar capture against the documented protocol (proves or breaks it)."""
import struct
from collections import Counter

PATH = r"D:\Development\fdad_radar\capture\samples\radar-live-sample.tsv"

def crc16(d):
    c = 0xFFFF
    for x in d:
        c ^= x
        for _ in range(8):
            c = (c >> 1) ^ 0xA001 if (c & 1) else c >> 1
    return c

types = Counter(); lens = Counter(); rec_n = []; crc_ok = crc_bad = frames = 0
trail = Counter(); meta = Counter(); rec32 = Counter(); samples = []

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
        pl = b[8:8 + L - 2]; stored = int.from_bytes(b[8 + L - 2:8 + L], "little")
        (crc_ok := crc_ok + 1) if crc16(pl) == stored else (crc_bad := crc_bad + 1)
        frames += 1
        t = int.from_bytes(pl[:4], "big"); types[t] += 1; lens[L] += 1
        if len(pl) >= 8: meta[int.from_bytes(pl[4:8], "big") & 3] += 1
        if t == 0x00030002 and len(pl) >= 48:
            body = pl[8:]
            rec = body[32:]
            n = int.from_bytes(rec[:8], "big"); rec_n.append(n)
            if n and 8 + n * 64 <= len(rec):
                for k in range(n):
                    r = rec[8 + k * 64: 8 + k * 64 + 64]
                    dw = struct.unpack(">16I", r)
                    for j in range(8, 16): trail[dw[j]] += 1       # bytes 32..64
                    for j in range(8):     rec32[(j, dw[j])] += 1  # first 8 dwords
            if len(samples) < 2:
                samples.append((L, n, len(rec), rec[:8].hex(), (rec[8:8+32]).hex()))

print("frames=%d  crc_ok=%d  crc_bad=%d" % (frames, crc_ok, crc_bad))
print("types:", {hex(k): v for k, v in types.most_common()})
print("LEN top:", lens.most_common(8))
print("meta (counter & 3):", dict(meta))
print("record counts seen:", Counter(rec_n).most_common(10))
print("record trailing dwords (bytes 32..64) top:", trail.most_common(8))
for s in samples:
    L, n, rl, cnt8, first32 = s
    print("sample: LEN=%d count=%d recfield=%dB count_bytes=%s rec0..32=%s" % (L, n, rl, cnt8, first32))
    print("        LEN-2 == 48 + count*64 ?  %d == %d  -> %s" % (L - 2, 48 + n * 64, (L - 2) == 48 + n * 64))
print("done")

