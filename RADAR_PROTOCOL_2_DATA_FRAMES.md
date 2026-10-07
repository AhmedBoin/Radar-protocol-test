# Radar Protocol — Part 2: Data Channel (header, CRC, frame types)

> ⚠️ **LEGACY (UDP generation) — NOT this radar.** See **[`PROTOCOL.md`](PROTOCOL.md)** for
> the real **TCP** protocol used by the BWR‑T15. Kept for reference only.


Legend: **[C]** confirmed in code · **[P]** inferred · **[?]** unknown.

## 1. Receiver & validation  **[C]**
`0x4D06E0` (uses `hasPendingDatagrams` / `pendingDatagramSize` / `readDatagram`).

```
Lp = readDatagram(...)
if Lp  <= 0x10            -> drop
LEN = u16 @ (data+4)
if LEN <= 0x14            -> error(1)      // 0x8D36B0(1)
if Lp  <= 0x14            -> error         // 0x8D36B0(1)
stored = u16 @ (data+LEN-2)
if crc16_modbus(data, LEN-2) != stored -> error(5)  // 0x8D36B0(5)
forward(data, LEN)        -> frame dispatcher 0x4CFBA0
```
Minimum valid frame length = 0x15 (21) bytes.

## 2. CRC-16/MODBUS  **[C]**
Poly `0xA001` (reflected 0x8005), init `0xFFFF`, reflected in/out, no final XOR.
Covers `data[0 .. LEN-2)`. Stored as 2 bytes at the end of the frame.

```c
uint16_t crc16_modbus(const uint8_t *p, int n){
    uint16_t crc = 0xFFFF;
    for (int i=0;i<n;i++){
        crc ^= p[i];
        for (int b=0;b<8;b++)
            crc = (crc & 1) ? (crc >> 1) ^ 0xA001 : (crc >> 1);
    }
    return crc;
}
```

## 3. Frame header  **[C]** — decoded by `0x904430` (BigEndian)

```
off  size  field
0    1     b0         u8      — [?] unit/address
1    1     b1         u8      — [?] flags
2    2     TYPE       u16 BE  — dispatched (see §4)
4    2     LENGTH     u16 BE  — TOTAL frame length (header+payload+crc)
6    2     b6         u16 BE  — [?] frame/session counter
8    8     TIMESTAMP  i64 BE  — time base (Part 3 §1)
16   LEN-18 PAYLOAD
LEN-2  2   CRC16               (u16)
```
* Header = **16 bytes**. Payload size = `LEN - 18`. (16 + (LEN-18) + 2 = LEN ✔)
* All multi-byte fields **big-endian** in the header/payload parse.

## 4. Frame dispatch  **[C]** — `0x4CFBA0`

```
type = decode(frame)                  // 0x904430
switch(type):
   3     -> 0x4CFCC0   52-byte (0x34) record  -> emit signal 0x8D36F0
   4     -> 0x4CFDE0   64-byte record decode  -> emit signal(s) 0x8D3760…0x8D37A0
   0x22  -> 0x4CFD10   QList of records       -> emit per-item 0x8D37A0
   default -> ignore
```

## 5. Type‑4 payload layout  **[C]** (`0x4CFE90..0x4CFFFC`, big‑endian)

| # | type | struct off | note |
|---|---|---|---|
| 1 | 64 bytes (raw) | `+0x00` | id/name buffer |
| 2 | `i32` | `+0x40` | |
| 3 | `i32` | `+0x44` | |
| 4 | `f64` | `+0x48` | |
| 5 | `f64` | `+0x50` | |
| 6 | `f64` | `+0x58` | |
| 7 | `i32` | `+0x60` | |
| 8 | `f64` | `+0x68` | |
| 9 | `i8`  | `+0x70` | |
| 10 | `f64` | `+0x78` | |
| 11 | `f64` | `+0x80` | |
| 12 | `i8`  | `+0x88` | |
| 13 | `i8`  | `+0x89` | |
| 14 | 16 bytes raw | `+0x8A` | |
| 15 | 16 bytes raw | `+0x9A` | |

**Type 3**: a fixed 52-byte (0x34) record is zeroed, filled, copied and emitted.
**Type 0x22**: a `QList` of records; each item passed to `0x8D37A0`.

> Bind the numeric fields to their names (from Part 3 §3) with a capture — the
> **shapes/types are confirmed**, the *labels* are **[P]**.

## 6. Notes / open items
1. Exact meaning of header bytes `b0, b1, b6` **[?]**.
2. Type‑3 and type‑0x22 record field maps **[P]** (only type‑4 decoded so far).
3. Whether target data arrives on UDP (seen) or also TCP (`QTcpSocket` @ `0x8D48E7`,
   `0x94E772`) — confirm by capture.
4. Capture with Wireshark on ports 5001/5002 to validate and label everything.
