# HACKING PLAYBOOK — when the radar is connected

Goal: sit in the middle of the app↔radar link, capture **both directions**, decode
against the RE'd protocol, and prove (or correct) every field.

## TL;DR — one command each step
```powershell
cd D:\Work\fdad-radar-protocol\capture

# 0. start FDAD-DCTv3.0.0(_EN).exe and log in
# 1. begin capturing (hooks the app's socket layer)
powershell -ExecutionPolicy Bypass -File .\monitor.ps1 -Mode start
# 2. connect the radar over Ethernet; in the app connect / change settings / start / stop
# 3. check progress at any time
powershell -ExecutionPolicy Bypass -File .\monitor.ps1 -Mode status
powershell -ExecutionPolicy Bypass -File .\monitor.ps1 -Mode tail
# 4. stop + decode
powershell -ExecutionPolicy Bypass -File .\monitor.ps1 -Mode stop
powershell -ExecutionPolicy Bypass -File .\monitor.ps1 -Mode decode
```

## What we are watching for (protocol checkpoints)

### Control channel — app → radar (expect UDP)
`7E 7E | LEN(u16 BE) | CMD | PAYLOAD | SUM&0xFF | 0D 0A`
- CMD should be one of the remapped wire codes: **0x21** (internal 0x1A) or **0x23** (internal 0x2A).
- Decoder verifies the checksum and prints the payload hex. Bind the payload field order:
  working mode, scan/angle/height/speed/RCS ranges, cfar, freq, etc.
- Watch for the **start / stop** orders (we will diff the payloads).

### Data channel — radar → app (expect UDP)
16-byte header + payload + **CRC-16/MODBUS**:
```
b0(u8) b1(u8) TYPE(u16 BE) LEN(u16 BE) b6(u16 BE) TIMESTAMP(i64 BE) | payload | CRC16(LE)
```
- TYPE should be **3**, **4**, or **0x22**.
- Decoder verifies the CRC and prints type/ts/payload.
- Bind: `b0`, `b1`, `b6`, and the type-3 / 0x22 record layouts. Track/target fields:
  tid, frame/scan, velocity(+dir), distance/range, azimuth, height, snr, rcs, type,
  beam, doppler, dw, pulse, ri, di, priv*.

### Incoming control ACK (radar → app)
`0xA1` / `0xA2` — small status blocks (decode their field order).

## Things to settle from the live capture
1. **Transport truth** — is the BWR-T15 really UDP (7E 7E + data CRC), or the app's
   separate **TCP `bwr::net` register** stack? (config says proto TCP on port 5001).
   The `CONNECT`/`BIND` events in the capture will show every socket + peer.
2. Header bytes `b0/b1/b6` meaning.
3. Field order inside command `0x21` / `0x23` (start vs stop, params).
4. `0xA1` / `0xA2` field semantics.
5. Type-3 and `0x22` payload maps (type-4 already decoded).

## Notes
- `monitor.ps1 -Mode start` attaches to the already-running app; **no config change**,
  and it sees the actual bytes regardless of remote IP/port (socket-boundary capture).
- Raw pcap alternative (elevated shell) if you also want a Wireshark pcap:
  `radar_sniff.ps1 -Mode start|stop|convert` (filters UDP 5002 + TCP 5001).
- If Frida shows nothing, the app may only send after a radar session is established —
  make sure the radar is cabled and "connected" in the UI.
- Keep `radar-live.tsv` — send it to me and I'll bind all remaining fields.
