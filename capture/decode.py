#!/usr/bin/env python3
"""decode.py - decode a radar_socketdump log (or any hex payload list) against the
RE'd radar protocol, so you can verify frames immediately.

Usage:
  python decode.py radar-capture-YYYYmmdd_HHMMSS.tsv
  python decode.py --hex 7e7e000001...           # decode a single hex blob

It recognises:
  * CONTROL frames  : 7E 7E | LEN(u16 BE) | CMD | PAYLOAD | SUM&0xFF | 0D 0A   (UDP, app->radar)
                      plus the uplink variant where CMD sits one byte later (offset 5).
  * DATA frames     : 16-byte header | payload | CRC-16/MODBUS (LE)            (UDP, radar->app)
                      header: b0(u8) b1(u8) TYPE(u16 BE) LEN(u16 BE) b6(u16 BE) TS(i64 BE)
"""
import sys, argparse

WIRE_TO_NAME = {0x21: 'CTRL_1A(0x21)', 0x23: 'CTRL_2A(0x23)'}


def crc16_modbus(data: bytes) -> int:
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if (crc & 1) else (crc >> 1)
    return crc


# ---- TCP radar protocol (the one this BWR-T15 actually uses) ----------------
# 55 AA 55 AA | LEN(u32 BE) | PAYLOAD(LEN-2) | CRC16-MODBUS(payload) (2B)
WIRE_TO_NAME = {0x21: 'CTRL_1A(0x21)', 0x23: 'CTRL_2A(0x23)',
                1: 'HEARTBEAT?', 2: 'MSG2?', 3: 'MSG3?'}


def try_tcp(b: bytes):
    if len(b) >= 12 and b[0] == 0x55 and b[1] == 0xAA and b[2] == 0x55 and b[3] == 0xAA:
        L = int.from_bytes(b[4:8], 'big')
        if 8 + L <= len(b) and L >= 2:
            payload = b[8:8 + L - 2]
            stored = b[8 + L - 2:8 + L]
            ok = 'OK' if crc16_modbus(payload) == int.from_bytes(stored, 'little') else 'BAD'
            t = int.from_bytes(payload[0:4], 'big')
            extra = ''
            if t == 1 and len(payload) >= 16:
                extra = ' counter=%d' % int.from_bytes(payload[4:8], 'big')
            return ('TCP len=%d pay=%d type=0x%08X crc=%s%s | %s'
                    % (L, len(payload), t, ok, extra, payload.hex()[:90]))
    return None



def try_control(b: bytes):
    if len(b) >= 8 and b[0] == 0x7E and b[1] == 0x7E:
        L = int.from_bytes(b[2:4], 'big')
        # outgoing layout: CMD @4
        for cmd_off, tag in ((4, 'down'), (5, 'up')):
            need = cmd_off + 1 + L + 1 + 2
            if len(b) >= need and b[need - 2] == 0x0D and b[need - 1] == 0x0A:
                cmd = b[cmd_off]
                payload = b[cmd_off + 1:need - 3]
                chk = b[need - 3]
                calc = sum(b[2:need - 3]) & 0xFF
                ok = 'OK' if chk == calc else 'BAD(%02x!=%02x)' % (chk, calc)
                name = WIRE_TO_NAME.get(cmd, 'cmd=0x%02X' % cmd)
                return 'CTRL[%s] %-14s len=%d chk=%s payload=%s' % (
                    tag, name, L, ok, payload.hex())
    return None


def try_data(b: bytes):
    if len(b) >= 0x15:
        L = int.from_bytes(b[4:6], 'big')
        if 0x15 <= L <= len(b):
            expected_le = crc16_modbus(b[:L - 2])
            stored_le = int.from_bytes(b[L - 2:L], 'little')
            stored_be = int.from_bytes(b[L - 2:L], 'big')
            if expected_le == stored_le or expected_le == stored_be:
                b0, b1 = b[0], b[1]
                typ = int.from_bytes(b[2:4], 'big')
                b6 = int.from_bytes(b[6:8], 'big')
                ts = int.from_bytes(b[8:16], 'big', signed=True)
                payload = b[16:L - 2]
                return ('DATA type=%d len=%d b0=0x%02X b1=0x%02X b6=%d ts=%d '
                        'payload(%d)=%s' % (typ, L, b0, b1, b6, ts, len(payload),
                                            payload.hex()[:120]))
    return None


def decode_blob(b: bytes, label: str):
    r = try_tcp(b) or try_control(b) or try_data(b)
    if r:
        print('%-6s %s' % (label, r))
        # also try to split concatenated control frames
        if b[:2] == b'\x7e\x7e':
            off = 0
            while off + 8 <= len(b) and b[off:off + 2] == b'\x7e\x7e':
                L = int.from_bytes(b[off + 2:off + 4], 'big')
                total = 4 + 1 + L + 1 + 2
                sub = b[off:off + total]
                s = try_control(sub)
                if not s:
                    break
                off += total
        return
    print('%-6s raw     len=%d %s' % (label, len(b), b.hex()[:120]))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('log', nargs='?', help='radar_socketdump TSV log')
    ap.add_argument('--hex', help='decode a single hex blob')
    a = ap.parse_args()

    if a.hex:
        decode_blob(bytes.fromhex(a.hex.replace(' ', '')), 'blob')
        return
    if not a.log:
        ap.error('give a log file or --hex')

    with open(a.log, encoding='utf-8', errors='replace') as f:
        counts = {}
        for line in f:
            parts = line.rstrip('\n').split('\t')
            if len(parts) < 5:
                if line.strip():
                    print('EVENT ' + line.strip())
                continue
            ts, direction, api, ln, hexs = parts[0], parts[1], parts[2], parts[3], parts[4]
            peer = parts[6] if len(parts) > 6 else '-'
            hexs = (hexs or '').strip()
            counts[(direction, api)] = counts.get((direction, api), 0) + 1
            if not hexs:                      # connect/bind event (no payload)
                print('%s %-7s %-9s %s' % (ts[-9:], direction, api, peer))
                continue
            try:
                b = bytes.fromhex(hexs)
            except ValueError:
                continue
            print('%s %-7s %-9s %-21s' % (ts[-9:], direction, api, peer), end=' ')
            decode_blob(b, direction)
        print('\n--- summary ---')
        for (d, a_), n in sorted(counts.items()):
            print('  %-8s %-10s %d' % (d, a_, n))


if __name__ == '__main__':
    main()
