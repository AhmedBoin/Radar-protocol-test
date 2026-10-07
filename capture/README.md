# Radar capture kit

Tools to capture and decode the FDAD-DCT radar protocol while the radar is connected,
so we can confirm every field of `fdad-radar-protocol` against real traffic.

## Files
| File | Purpose |
|---|---|
| `radar_sniff.ps1` | one command to capture (Frida payloads **or** pktmon pcap) and decode |
| `radar_socketdump.js` | Frida script: dumps every TCP+UDP send/recv payload of the app |
| `decode.py` | decodes the captured payloads using the confirmed protocol |

## A. Payload capture (recommended) — Frida, no Wireshark, no config change
1. Start the app (`D:\UAV Radar\FDAD-DCTv3.0.0.exe`) and log in.
2. In a **new PowerShell**:
   ```powershell
   cd D:\Work\fdad-radar-protocol\capture
   powershell -ExecutionPolicy Bypass -File .\radar_sniff.ps1 -Mode frida
   ```
   It attaches to the running app and writes `radar-capture-<stamp>.tsv`
   (tab-separated: `epoch_ms  DIR  API  LEN  HEX  ASCII`).
3. **Connect the radar** over Ethernet, connect in the app, let it run ~30–60 s.
4. `Ctrl+C` to stop.
5. Decode it against the protocol:
   ```powershell
   .\radar_sniff.ps1 -Mode decode
   ```
   Every CONTROL (`7E 7E …`) and DATA (16-byte header + CRC-16/MODBUS) frame is
   recognised, checksum/CRC verified, and printed. Send me the `.tsv` (and the decode
   output) and I'll bind every field name.

## B. Raw packet capture — built-in pktmon (needs an **elevated** shell)
```powershell
.\radar_sniff.ps1 -Mode start                 # filters: UDP 5002 + TCP 5001, capture -> .etl
# ... connect the radar, reproduce ...
.\radar_sniff.ps1 -Mode stop
.\radar_sniff.ps1 -Mode convert               # .etl -> .pcap
```
Open the `.pcap` in Wireshark and filter `ip.addr == 192.168.8.167`.

## What we are looking for
* **Control (app→radar, UDP):** `7E 7E | LEN(u16 BE) | CMD | PAYLOAD | SUM&0xFF | 0D 0A`
  — CMD bytes `0x21` / `0x23`; bind the payload field order (working mode, scan/angle/
  height/speed/RCS ranges, cfar, freq).
* **Data (radar→app, UDP):** 16-byte header (`b0 b1 TYPE(u16 BE) LEN(u16 BE) b6(u16 BE)
  TIMESTAMP(i64 BE)`) + payload + **CRC-16/MODBUS** — types `3`, `4`, `0x22`.
* Confirm **which transport** the BWR-T15 uses (the app config says `proto: TCP`, but the
  RE'd control path uses UDP — capture settles it).

## Requirements
* Frida (`frida-tools`) — installed. Python — installed.
* pktmon (built-in) + elevation — for the pcap path only.
