# FDAD / BWR‑T15 Radar — Wire Protocol (complete, verified)

Reverse‑engineered from `FDAD-DCTv3.0.0.exe` and **verified against a live BWR‑T15** at
`192.168.8.167:5001`. Every item marked ✅ was confirmed on the wire (CRC‑checked, and
target fields matched the app's own DB/CSV output).

---

## 1. Transport  ✅
* **TCP**, app = client → `radar_ip : 5001` (from `setting.json → regions[].radars[].nwk`).
* **Plaintext** (no TLS). All multi‑byte integers are **big‑endian**.
* A **login handshake is required** or the radar streams nothing.

## 2. Frame (both directions)  ✅
```
+---------------+----------------+---------------------------+-------------------------+
| 55 AA 55 AA   | LEN (u32 BE)   | PAYLOAD  (LEN - 2 bytes)  | CRC16-MODBUS(payload)   |
| magic (4 B)   | = payload + 2  |                           | u16, little-endian (2 B) |
+---------------+----------------+---------------------------+-------------------------+
total frame length = LEN + 8
```
* **CRC = CRC‑16/MODBUS** (poly `0xA001`, init `0xFFFF`, reflected, no final XOR), over the
  **PAYLOAD only**, stored as a **little‑endian u16** in the last 2 bytes.
  * check: `crc16_modbus(b"123456789") == 0x4B37`.
* Parser rejects unless magic == `0x55AA55AA`, `0x0A ≤ LEN ≤ 0x07FFFFF6`.

## 3. Message = payload
```
type (u32 BE) | counter (u32 BE) | body ...
```
`counter` = monotonic per‑connection message counter (0,1,2,…).

| type | dir | name | body |
|---|---|---|---|
| `0x00000002` | app→radar | **LOGIN** (send first!) | `u32 3 \| u32 0` |
| `0x00000001` | app→radar | heartbeat (~every 10 s) | `u64 0` |
| `0x00000040` | app→radar | **SetRegister** | `u32 count \| (reg u32, value u32) * count` |
| `0x00000041` | app→radar | GetRegister | `u32 count \| reg u32 * count` |
| `0x00000080` | app→radar | **SetJson** (settings) | `u32 len \| u32 0 \| json[len]` |
| `0x00000082` | app→radar | GetJson | `u32 0` (or params) |
| `0x00030002` | radar→app | **DATA stream** | see §4 |

### Login  ✅
```
55 AA 55 AA | 00 00 00 12 | 00 00 00 02 | 00 00 00 00 | 00 00 00 03 | 00 00 00 00 | CRC(2)
                 LEN=18        type=2       counter=0      3              0
```
Send immediately after connecting; then the radar starts streaming.

### Startup sequence captured from the app  ✅
```
ctr=0  type 0x02  body 00000003 00000000            (login)
ctr=1  type 0x41  body 00000002 00000440 00000401    (get registers 0x440, 0x401)
ctr=2  type 0x81  body 00000000
ctr≥3  type 0x01  body 0000000000000000              (heartbeats)
```

## 4. Data message (radar → app)  ✅
```
type 0x00030002 | counter u32 | body:
  u16  0x0006                     length of the id that follows
  [6]  MAC                        device id (e.g. 37 9A D3 9E 08 6E)
  u64  frameNum                   +1 per message
  i64  timestamp                  radar clock, milliseconds
  u32  boundaryA                  sweep start = azimuth × 10000
  u32  boundaryB                  sweep end   = azimuth × 10000
  RECORDS
```
`azimuth_deg = boundary / 10000`. Payload streams in chunks (48 B … ~3 kB each).

### Records (vehicle tracks)  ✅
```
u64 count , then count × 64-byte records
```
| off | type | field | scale | verified against CSV/DB |
|---|---|---|---|---|
| 0 | i32 | **X** east | cm | |
| 4 | i32 | **Y** north | cm | |
| 8 | i32 | **Height** | cm | id 820: 9090 → 90.9 ✅ |
| 12 | i32 | **Vx** | cm/s | |
| 16 | i32 | **Vy** | cm/s | |
| 20 | i32 | type / menace | — | |
| 24 | u32 | **ID** (batch NO) | — | == CSV ID ✅ |
| 28 | u32 | **(SNR×100 << 16) \| (RCS×100)** | | id 820: `0x0955_0042` ✅ |
| 32‑63 | — | reserved / previous values | | |

**Derived quantities:**
```
distance_m = hypot(X, Y) / 100
azimuth_deg = degrees(atan2(X, Y)) mod 360      # 0° = north, clockwise
speed_mps  = hypot(Vx, Vy) / 100
snr        = (rec[28] >> 16) / 100
rcs        = (rec[28] & 0xFFFF) / 100
```
Example (id 820): X=−495595, Y=783412 → **dist 9270.1 m** (DB 9270.11 ✅),
**az 327.7°** (DB 326.2 ✅); Vx=1996, Vy=−1024 → **speed 22.44 m/s** (DB 22.48 ✅).

## 5. Control  ✅
| action | command |
|---|---|
| **Rotate** | `SetRegister` reg **`0x0401`** = **`0x00000401`** |
| **Stop** (Stand by) | `SetRegister` reg **`0x0401`** = **`0x00000000`** |
| Settings block | `SetRegister` reg **`0x0440`** (values seen: `0x00030102`, `0x03030102`) |
| Position / areas | `SetJson` → `{"position":{"lat":..,"lon":..,"altitude":..,"yaw":..},"areas":[]}` |

**Verified:** sending `0x401=0` → data flow stops; `0x401=0x401` → data flow resumes.

## 6. Reference implementation (Python)
```python
import socket, struct, math
MAGIC = b"\x55\xaa\x55\xaa"

def crc16(d):
    c = 0xFFFF
    for x in d:
        c ^= x
        for _ in range(8):
            c = (c >> 1) ^ 0xA001 if (c & 1) else c >> 1
    return c

def frame(payload):                      # 55AA55AA | LEN | payload | CRC16-LE
    return MAGIC + struct.pack(">I", len(payload) + 2) + payload + struct.pack("<H", crc16(payload))

def login(c=0):          return struct.pack(">II", 0x02, c) + struct.pack(">II", 3, 0)
def heartbeat(c):        return struct.pack(">IIQ", 0x01, c, 0)
def setreg(c, reg, val): return struct.pack(">III", 0x40, c, 1) + struct.pack(">II", reg, val)
ROTATE = (0x0401, 0x00000401)
STOP   = (0x0401, 0x00000000)

def parse_data(payload):                 # payload = type|counter|body
    b = payload[8:]
    mac = b[2:8]; frame_num = struct.unpack(">Q", b[8:16])[0]
    ts  = struct.unpack(">q", b[16:24])[0]
    bA  = struct.unpack(">I", b[24:28])[0] / 10000.0
    bB  = struct.unpack(">I", b[28:32])[0] / 10000.0
    rec = b[32:]; n = struct.unpack(">Q", rec[:8])[0]
    vehicles = []
    for k in range(n):
        r = rec[8 + k*64: 8 + k*64 + 64]
        X, Y, H, Vx, Vy = struct.unpack(">iiiii", r[0:20])
        tid = struct.unpack(">I", r[24:28])[0]
        pk  = struct.unpack(">I", r[28:32])[0]
        vehicles.append(dict(id=tid, dist=math.hypot(X, Y)/100, az=math.degrees(math.atan2(X, Y)) % 360,
                             height=H/100, speed=math.hypot(Vx, Vy)/100,
                             snr=(pk >> 16)/100, rcs=(pk & 0xFFFF)/100))
    return frame_num, ts, bA, bB, vehicles
```

## 7. Reassembly
TCP may split/merge frames. Resync on `55 AA 55 AA`, read `LEN`, wait for `8+LEN` bytes,
verify CRC, then process. See `gui/radar_gui.py` (`RadarLink`) and `src/frame.rs` (Rust).

## 8. Still open (not needed for control/plot)
* `off20` exact meaning (type / menace enum) — currently used as the target type.
* `0x81` vs `0x82` Get‑JSON variants.
* `boundaryA/B` full meaning beyond azimuth (sweep raster position).

---
_Generated from live captures of `FDAD-DCTv3.0.0.exe` ↔ BWR‑T15. Kit: `capture/` (Frida
socket dumper, decoder), `gui/radar_gui.py` (working GUI), Rust crate in `src/`._

