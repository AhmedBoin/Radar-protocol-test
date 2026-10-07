#!/usr/bin/env python3
"""parse_frames.py - parse the radar TCP protocol out of a socketdump .tsv.

Frame:  55 AA 55 AA | LEN(u32 BE) | PAYLOAD(LEN-2) | CRC16-MODBUS(payload) (2B LE)

Data message (radar->app), payload header (40 bytes):
  u32  type = 0x00030002
  u32  counter                 (increments ~+4 per message)
  u16  0006 | [6] MAC (fixed)  device id
  u64  frameNum                (+1 per message)
  i64  timestamp               radar clock, ms
  u32  boundaryA               sector boundary (== DB track_frame.boundaryA)
  u32  boundaryB               sector boundary (== DB track_frame.boundaryB)
  ...  records

Heartbeat (app->radar): u32 type=1 | u32 counter | u64 0

Usage:  python parse_frames.py radar-live.tsv
"""
import sys

def crc16(d):
    c = 0xFFFF
    for x in d:
        c ^= x
        for _ in range(8):
            c = (c >> 1) ^ 0xA001 if (c & 1) else c >> 1
    return c

def parse(b):
    if len(b) < 12 or b[0] != 0x55 or b[1] != 0xAA or b[2] != 0x55 or b[3] != 0xAA:
        return None
    L = int.from_bytes(b[4:8], 'big')
    if 8 + L > len(b) or L < 2:
        return None
    pl = b[8:8 + L - 2]
    stored = int.from_bytes(b[8 + L - 2:8 + L], 'little')
    ok = (crc16(pl) == stored)
    t = int.from_bytes(pl[0:4], 'big')
    if t == 1:
        return ('HEARTBEAT crc=%s counter=%d' % (ok, int.from_bytes(pl[4:8], 'big')), pl)
    if t == 0x00030002 and len(pl) >= 40:
        ctr = int.from_bytes(pl[4:8], 'big')
        mac = ':'.join('%02x' % x for x in pl[10:16])
        fn = int.from_bytes(pl[16:24], 'big')
        ts = int.from_bytes(pl[24:32], 'big', signed=True)
        ba = int.from_bytes(pl[32:36], 'big')
        bb = int.from_bytes(pl[36:40], 'big')
        rec = pl[40:]
        return ('DATA crc=%s ctr=%d mac=%s frameNum=%d ts=%d bA=%d bB=%d rec=%dB'
                % (ok, ctr, mac, fn, ts, ba, bb, len(rec)), rec)
    return ('type=0x%08X crc=%s len=%d' % (t, ok, len(pl)), pl)

def main():
    path = sys.argv[1] if len(sys.argv) > 1 else 'radar-live.tsv'
    n = dat = 0
    with open(path, encoding='utf-8', errors='replace') as f:
        for line in f:
            p = line.rstrip('\n').split('\t')
            if len(p) < 5:
                continue
            hexs = (p[4] or '').strip()
            if not hexs:
                continue
            try:
                b = bytes.fromhex(hexs)
            except ValueError:
                continue
            r = parse(b)
            if not r:
                continue
            n += 1
            if n <= 6 or n % 500 == 0:
                print('%-4s %-7s %s' % (p[1], p[2], r[0]))
            dat += 1
    print('--- parsed %d protocol frames ---' % dat)

if __name__ == '__main__':
    main()
