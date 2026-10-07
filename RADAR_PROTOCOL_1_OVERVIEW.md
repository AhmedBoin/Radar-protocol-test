# Radar Protocol — Part 1: Overview & Control Channel

> ⚠️ **LEGACY (UDP generation) — NOT the protocol this BWR‑T15 uses.** This describes an
> older UDP radar generation found in the binary. The actual radar uses the **TCP**
> protocol in **[`PROTOCOL.md`](PROTOCOL.md)**. Kept for reference only.


Applies to `FDAD-DCTv3.0.0.exe` (PE32, MinGW GCC, Qt 5). Derived by static analysis.
Legend: **[C]** confirmed in code · **[P]** inferred (verify with capture) · **[?]** unknown.

## 0. Quick reference

| Item | Value |
|---|---|
| Control channel | **UDP** (app `udpPort`, default 5002) |
| Data channel | **UDP** by default (a TCP path also exists, `tcpPort` default 5001) **[P]** |
| Control framing | `7E 7E …`, big-endian u16, 8-bit checksum, ends `0D 0A` |
| Data framing | 16-byte header + payload + **CRC-16/MODBUS** (Part 2) |
| Data frame types | `3`, `4`, `0x22` |
| Endpoint config | `setting.json → regions[].radars[] { ip, port(5001), proto }` |

---

## 1. Control channel — `7E 7E` framing  **[C]**

Builder `sub_4C4910` @ `0x4C4910`; parser `sub_4C4AB0` @ `0x4C4AB0`.
Both use `QDataStream` with **`setByteOrder(BigEndian)`**.

```
offset  size  field
0       1     0x7E                    (SOF)
1       1     0x7E                    (SOF)
2       2     LENGTH  (u16 big-endian)  = payload byte count
4       1     CMD
5       L     PAYLOAD
5+L     1     CHECKSUM = (sum of bytes from offset 2 .. end of payload) & 0xFF
6+L     1     0x0D
7+L     1     0x0A
```
Total size = `8 + L`.

### 1.1 Outgoing command codes (app → radar)
Internal code is remapped to the wire byte:

| internal | wire CMD | builder call-site |
|---|---|---|
| `0x1A` | **`0x21`** | `0x4C3D8F`, `0x4C549C` |
| `0x2A` | **`0x23`** | `0x4C3BDF`, `0x4C52D0` |

### 1.2 Incoming command codes (radar → app)
Parser returns the CMD byte; UDP receiver dispatches on:

| CMD | handler |
|---|---|
| **`0xA1`** | `0x4C4660` |
| **`0xA2`** | `0x4C45C2` |
| other | ignored |

### 1.3 Parser flow (`sub_4C4AB0`)
```
ds = QDataStream(frame); ds.setByteOrder(BigEndian)
u16 -> SOF (0x7E7E)
u16 -> LENGTH
u8  -> b4                 (extra byte, present on uplink)
u8  -> CMD                <-- returned to caller
readRawData(LENGTH)  -> payload (appended to out QByteArray)
u8  -> checksum
u16 -> 0x0D0A
```
> Builder writes CMD at offset 4; parser reads CMD at offset 5 → the **radar→app frame
> has one extra byte at offset 4** (address/target) **[P]**.

### 1.4 Payload field types (status/ACK) **[C]**
* **CMD 0xA1** → object fields at `+0x98`: `i8, i8, i16, i16, i16, i16, i16, i8, i16`
* **CMD 0xA2** → object fields at `+0xAC`: `i16, i8, …`

These are control/status, **not** target data.

---

## 2. Document map
* **Part 1 (this)** — transport, control framing, commands.
* **Part 2** — data framing, CRC-16/MODBUS, header, frame types, payload structs.
* **Part 3** — time base, scan/speed parameters, target & alarm data models.

---

## 3. Key code addresses (control)

| What | VA |
|---|---|
| Packet builder | `0x4C4910` |
| Packet parser | `0x4C4AB0` |
| Control RX (UDP) | `0x4C4540` (dispatch `0x4C45B0`) |
| CMD 0xA1 handler | `0x4C4660` |
| CMD 0xA2 handler | `0x4C45C2` |
| UDP send sites | `0x4C3C10`, `0x4C550C` |
