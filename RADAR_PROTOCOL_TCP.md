# RADAR PROTOCOL (TCP) — confirmed from live capture  [★ = verified on the wire]

Model **BWR‑T15**, app `FDAD-DCTv3.0.0(.exe)`. Validated 2026‑10‑07 against a live radar
at `192.168.8.167:5001` (capture kit in `capture/`).

> ⚠️ **Important:** the earlier UDP `7E 7E …` design in `RADAR_PROTOCOL_1/2/3_*.md` is a
> **different (older) radar generation**. THIS radar (BWR‑T15) speaks the **TCP** protocol
> below. (Both exist in the binary; the app picked TCP, and there were **no UDP sockets**.)

## 1. Transport  [★]
* **TCP**, app = client → **radar IP : 5001** (from `setting.json → regions[].radars[].nwk`).
* App also opens HTTP to a tile/map server (unrelated). **No UDP** used for the radar.
* No TLS (plaintext).

## 2. Frame (both directions)  [★]
```
+----------+----------------+---------------------------+---------------------+
| 55 AA 55 | LEN (u32 BE)   | PAYLOAD  (LEN - 2 bytes)  | CRC16-MODBUS (2 B)  |
| AA (magic| = payload+2    |                           | over PAYLOAD, u16 LE |
+----------+----------------+---------------------------+---------------------+
```
* **Total frame length = LEN + 8.**
* Multi‑byte ints are **big‑endian** (parser uses `ntohl`; sender `setByteOrder(0)` = BigEndian).
* Parser (`0x4D0F37` region) checks: magic `0x55AA55AA`, `LEN ≥ 0x0A`, `LEN ≤ 0x7FFFFF6`,
  then `call 0x4D9320` (CRC) which returns **0 when valid** (residual check).
* **CRC = CRC‑16/MODBUS** (poly `0xA001`, init `0xFFFF`, reflected, no final XOR),
  computed over **PAYLOAD only**, stored **little‑endian** as the last 2 bytes.
  * e.g. heartbeat payload `000000010000005e0000000000000000` → `crc16=0x120C` → bytes `0C 12`. [★]
* Sender builder at **`0x4D0D10`** (writes magic, `LEN=payload+2`, payload, CRC).
  CRC helper **`0x4D9320`** (tables at `0xa63b80`/`0xa63c80`).

## 3. Messages observed  [★]

### 3.1 Heartbeat — app → radar  (every ~10 s)
```
PAYLOAD (16 B):  TYPE(u32 BE)=1 | COUNTER(u32 BE) | 0x0000000000000000 (u64)
```
`TYPE = 1`, `COUNTER` increments by 1 each beat. `LEN = 18`.
Example: `55AA55AA 00000012 00000001 0000005E 0000000000000000 0C12` [★ crc OK]

### 3.2 Data — radar → app  (continuous, the sweep stream)
`TYPE` field (first u32) = **`0x00030002`** for the data stream. Payload header after the u32:
```
u16  0003           \
u16  0002           /  -> the u32 "type" 0x00030002
u32  counter            (increments ~+4 each message)
u16  0006               length of the device id that follows
[6]  MAC                37 9A D3 9E 08 6E   (fixed device id) [★]
u64  frameNum           +1 per message  (1A16, 1A17, …)            [≈ track_frame.frameNum]
i64  timestamp          epoch **milliseconds**, +~90 ms/message    [≈ track_frame.refreshTime]
u32  boundaryA          sector boundary (slides: A(n)=B(n-1))       [≈ track_frame.boundaryA]
u32  boundaryB          sector boundary (increments each message)   [≈ track_frame.boundaryB]
...  records            track / detection records (see §4)
```
Payload length varies per message (48 … ~3000 B) → the sweep is streamed in chunks.

## 4. Open items (to bind with the running capture)
1. The **record layout** inside the data payload (track fields): tid, velocity(+dir),
   range/distance, azimuth, height, snr, rcs, type, beam, doppler, dw, pulse, ri, di,
   priv* — align against the DB schema (`track1/2/3_data`, `cfar_data`).
2. **Command frames** (change settings / start / stop / mode) — only heartbeats seen so far;
   operate the app and capture the app→radar command payloads (`TYPE ≠ 1`).
3. The exact meaning of `counter` (u32) and the two `LEN`-prefixed / boundary fields.

> Note: the crate's `data.rs` (16‑byte header + CRC over whole frame) and `control.rs`
> (`7E 7E` UDP) do **not** match this radar; keep them only for the legacy generation.
