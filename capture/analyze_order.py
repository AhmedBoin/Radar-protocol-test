#!/usr/bin/env python3
"""Print the first frames of the capture in order (the handshake/conversation)."""
P = r"D:\Development\fdad_radar\capture\samples\radar-live-sample.tsv"
n = 0
for line in open(P, encoding="utf-8", errors="replace"):
    p = line.rstrip("\n").split("\t")
    if len(p) < 5:
        continue
    d, hx = p[1], (p[4] or "").strip()
    if not hx:
        print("%-5s (event) %s" % (d, p[6] if len(p) > 6 else ""))
        continue
    try:
        b = bytes.fromhex(hx)
    except ValueError:
        continue
    off = 0
    while off + 8 <= len(b) and b[off:off + 4] == b"\x55\xaa\x55\xaa":
        L = int.from_bytes(b[off + 4:off + 8], "big")
        if L < 2 or off + 8 + L > len(b):
            break
        pl = b[off + 8:off + 8 + L - 2]
        if len(pl) >= 8:
            t = int.from_bytes(pl[:4], "big")
            ctr = int.from_bytes(pl[4:8], "big")
            body = pl[8:]
            print("%-5s type=0x%08X ctr=%-4d len=%-5d body(%d)=%s"
                  % (d, t, ctr, L, len(body), body[:40].hex()))
            n += 1
        off += 8 + L
    if n > 55:
        break
