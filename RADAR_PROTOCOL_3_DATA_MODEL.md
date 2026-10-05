# Radar Protocol — Part 3: Time Base, Scan/Speed, Target Data

Legend: **[C]** confirmed in code · **[P]** inferred · **[?]** unknown.

## 1. Time base  **[C/P]**

Every **data frame** carries a timestamp and is frame-indexed, so the stream is a
monotonic, per-sweep sequence.

| Source | Field | Type | Meaning |
|---|---|---|---|
| Frame header @ off 8 | `TIMESTAMP` | `i64` BE | frame time (radar clock) — ms **[P]** |
| Frame header @ off 6 | `b6` | `u16` BE | frame/session counter **[?]** |
| DB `track_frame` | `frameNum` | i32 | sweep index (once per revolution) |
| DB `track_frame` | `realFrameNum` | i32 | raw radar frame index (no gaps dropped) |
| DB `track_frame` | `refreshTime` | i64 | wall-clock time of the frame |

* Playback/queries use `refreshTime` windows:
  `WHERE refreshTime > ? AND refreshTime < ?`.
* Alarms store `startTime` / `endTime` (i64) + `frameNum`.
* **Rule:** `frameNum`+1 = next antenna revolution; `refreshTime` (i64) is the real
  timestamp used for display/playback. Prefer header `TIMESTAMP` (off 8) as the
  authoritative per-frame time in your reimplementation.

### Sweep period (inter-frame time) — from the working mode  **[C]**

| Working mode | Sweep period |
|---|---|
| `Cir sweep 1s` | 1 s |
| `Cir sweep 2s` | 2 s |
| `Cir sweep 3s` | 3 s |
| `Cir sweep 4s` | 4 s |
| `Fan sweep` | sector scan (not 360°) |
| `Stand by` | no rotation |

`setting.json → radarOption.trackDuration` = 2 s (trail length in seconds).

## 2. Speed scan / detection parameters  **[C/P]**

Per-radar `hardware{}` (in `setting.json`, used by the app):

| Field | Meaning | Unit |
|---|---|---|
| `workingMode` | 0 = circumference, 1 = fan … | enum |
| `scanRangeLow/High` | detection distance limits | m |
| `angleRangeLow/High` | azimuth sector | ° |
| `speedRangeLow/High` | velocity window | m/s |
| `rcsRangeLow/High` | RCS window | m² |
| `heightRangeLow/High` | height window | m |
| `cfar` | CFAR threshold | — |
| `freq` | frequency / channel | — |

`display{}`: `tail`, `site`, `aocLow/aocHigh`, `pitchWaveLevel`.
`nwk{}`: `ip`, `port`, `conv`, `proto`.

Radar models: `BWR-G12, BWR-G06, BWR-G03, BWR-G01, BWR-S01, BWR-A01, BWR-FP, BWR-T10,
BWR-T15, BWR-T20`.

UI strings mirroring these: `Detection Range:`, `Azimuth Coverage:`, `Center Direction:`,
`Velocity Threshold:`, `RCS Threshold:`, `Detection Threshold:`, `Trail Time:`,
`Pitch wave level:`, `Working Mode:`, `Scanning range:`, `Speed Range:`, `Height Range:`,
`RCS Range:`, `Angle Range:`.

> The **byte offset of each of these inside command `0x21`/`0x23`** is **[P]** —
> the payload is a serialized struct; confirm field order with a capture.

## 3. Target data model  **[C]** (from the app's SQLite schemas)

### 3.1 Frame (one sweep)
```
track_frame(tfID, tcID, version(i32), frameNum(i32), realFrameNum(i32),
            refreshTime(i64), boundaryA(f64), boundaryB(f64), direction(i32))
```
`boundaryA/B` = detection sector/raster boundaries; `direction` = antenna/boresight angle.

### 3.2 Targets (tracks) — three stored variants
```
track1_data(tid(i64), frameNum, version,
            velocity(f64), distance(f64), azimuth(f64), snr(f64),
            type(i32), beam(i32), doppler(i32), dw(i32), pulse(i32))

track2_data(... + velocityDirection(f64), range(f64),
            privDistance, privAzimuth, privVelocity, privVelocityDirection)

track3_data(tid, frameNum, version,
            velocity(f64), distance(f64), range(f64), azimuth(f64), height(f64),
            type(i32), snr(f64), rcs(f64), ri(i32), di(i32),
            beam(i32), privDistance, privAzimuth, privHeight)
```
Field meaning:
* `tid` — **track id (i64)**, stable across frames → the target key.
* `velocity` (m/s), `velocityDirection` (°), `distance`/`range` (m), `azimuth` (°),
  `height` (m), `snr` (dB), `rcs` (m²).
* `type` — classification (§3.4).
* `beam` (beam index), `doppler` (Doppler bin), `dw`, `pulse` (pulse-width index),
  `ri` / `di` (range/Doppler indices).
* `priv*` — the **previous frame's value** of the same quantity → used to draw trails.

### 3.3 CFAR plot cloud (raw detections, not tracks)
```
cfar_frame(cfID, tcID, frameNum, refreshTime, boundaryA(f64), boundaryB(f64), direction(i32))
cfar_data(cfID, distance(f64), azimuth(f64), height(f64), pitch(f64),
          doppler(i32), di(i32), ri(i32), snr(f64), beam(i32), pluse(i32))
```

### 3.4 Target classification (`type`)
`UnDetected`, `MultiBirds`, `Airplane`, `Drones`/`drone`, `Birds`, `Pedestrians`,
`Vehicles`, `Unknown`.

### 3.5 Alarms (derived, geo-referenced)
```
alarm_context(tid, alarmId, startTime(i64), endTime(i64), radarName, alarmArea, type)
alarm_data(alarmId, frameNum, lat(f64), lon(f64), height(f64), speed(f64))
```

### 3.6 Session records (local persistence, not on the wire)
```
terminal_context(tcID, rid, version, radarName, startTime, endTime,
                 startOperation, endOperation, CBPath, DBPath)
```

## 4. Reference: control frame build/parse

```python
# build (app -> radar), big-endian
def build(cmd, payload: bytes) -> bytes:
    body  = bytes([cmd]) + payload
    frame = b"\x7E\x7E" + len(payload).to_bytes(2, "big") + body
    chk   = sum(frame[2:]) & 0xFF
    return frame + bytes([chk, 0x0D, 0x0A])

# parse (radar -> app)
def parse(buf: bytes):
    ln  = int.from_bytes(buf[2:4], "big")
    cmd = buf[5]                 # note: offset 5 on the uplink
    return cmd, buf[6:6+ln]
```

## 5. Recommended validation
Capture on **UDP 5002 / TCP 5001** with Wireshark while commanding a radar, then bind
labels to the 0xA1/0xA2 fields and the type‑3/‑4/0x22 records against §3.

## 6. Key addresses

| What | VA |
|---|---|
| Data RX (CRC) | `0x4D06E0` |
| Frame dispatcher | `0x4CFBA0` |
| Frame header decoder | `0x904430` |
| Type‑3 / ‑4 / 0x22 handlers | `0x4CFCC0` / `0x4CFDE0` / `0x4CFD10` |
| Target `ds>>f64` parse | `0x4D272A` |
| Record/list parsers | `0x4D26A0`, `0x4D2790`, `0x4D1800`, `0x4D9400+` |
| SQLite schema strings | file `0x65B000` region |
