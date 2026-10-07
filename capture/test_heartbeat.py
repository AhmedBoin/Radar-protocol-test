#!/usr/bin/env python3
"""Test that radar_mimic replies to a heartbeat (type 0x01) and streams data."""
import socket, struct, time, collections


def crc16_modbus(data: bytes) -> int:
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc


def frame(payload: bytes) -> bytes:
    return (b"\x55\xaa\x55\xaa" + struct.pack(">I", len(payload) + 2)
            + payload + struct.pack("<H", crc16_modbus(payload)))


def msg(mtype: int, counter: int, body: bytes = b"") -> bytes:
    return frame(struct.pack(">II", mtype, counter) + body)


s = socket.create_connection(("127.0.0.1", 5001), timeout=5)
s.sendall(msg(0x02, 0))       # login
time.sleep(0.2)
s.sendall(msg(0x01, 11))      # heartbeat counter 11 (must be echoed back)

s.settimeout(2.0)
seen = collections.Counter()
first_hb = None
buf = b""
t0 = time.time()
while time.time() - t0 < 3:
    try:
        d = s.recv(65536)
    except socket.timeout:
        break
    if not d:
        break
    buf += d
    while len(buf) >= 8:
        if buf[:4] != b"\x55\xaa\x55\xaa":
            buf = buf[1:]
            continue
        L = struct.unpack(">I", buf[4:8])[0]
        if len(buf) < 8 + L:
            break
        pl = buf[8:8 + L - 2]
        buf = buf[8 + L:]
        t = struct.unpack(">I", pl[:4])[0]
        c = struct.unpack(">I", pl[4:8])[0]
        seen[t] += 1
        if t == 0x0000_0001 and first_hb is None:
            first_hb = (c, pl[8:].hex())
s.close()

print("types seen:", {hex(k): v for k, v in seen.items()})
print("heartbeat reply (counter, body):", first_hb)
print("PASS" if first_hb and first_hb[0] == 11 else "FAIL")
