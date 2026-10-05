# fdad-radar

A Rust crate implementing the **FDAD-DCT v3.0.0 radar interface protocol**
(reverse-engineered from `FDAD-DCTv3.0.0.exe`). Use it to build a new app that
**controls the radar and receives its data**.

* Zero dependencies — builds offline on stable Rust (`edition 2021`).
* `no_std`-free, `#![deny(missing_docs)]`, fully tested.

## Add it to your project

```toml
[dependencies]
fdad-radar = { path = "../fdad-radar-protocol" }   # or a git/path dependency
```

```rust
use fdad_radar::commands::OutgoingCommand;
use fdad_radar::control::ControlFrame;
use fdad_radar::transport::UdpRadar;
use std::time::Duration;

let radio = UdpRadar::connect("192.168.8.100:5002")?;
radio.set_read_timeout(Some(Duration::from_secs(2)))?;

// send a command
let cmd = ControlFrame::new(OutgoingCommand::Ctrl2A, &[/* payload */]);
radio.send_control(&cmd)?;

// receive + validate a data frame
let frame = radio.recv_data_frame()?;
println!("type={:?} ts={} bytes={}", frame.header.kind(),
         frame.header.timestamp, frame.payload.len());
# Ok::<(), fdad_radar::Error>(())
```

## Protocol summary

### Control channel (UDP) — module `control`
```
7E 7E | LEN(u16 BE) | CMD(u8) | PAYLOAD | CHECKSUM(u8) | 0D 0A
CHECKSUM = (sum of bytes from offset 2 .. end of payload) & 0xFF
```
* Outgoing: internal `0x1A` -> wire `0x21`, internal `0x2A` -> wire `0x23`.
* Incoming: `0xA1`, `0xA2` (see `commands::StatusA1`).
* Helpers: `ControlFrame::encode/decode`, `control::split_frames` for streams.

### Data channel (UDP) — module `data`
```
0: u8 b0 | 1: u8 b1 | 2: u16 TYPE | 4: u16 LEN | 6: u16 b6 | 8: i64 TIMESTAMP
16: payload (LEN-18) | LEN-2: u16 CRC16   (CRC-16/MODBUS over bytes[0..LEN-2])
```
* Frame types: `3`, `4`, `0x22` (`data::FrameType`).
* `data::DataFrame::parse` validates the CRC and splits header/payload.
* `data::Type4Record` decodes the type‑4 payload field-by-field.

### CRC — module `crc`
`crc::crc16_modbus(data)` — poly `0xA001`, init `0xFFFF`, no final XOR
(check value of `"123456789"` is `0x4B37`).

## Modules

| Module | Purpose |
|---|---|
| `control` | `7E 7E` control framing (`ControlFrame`, `IncomingFrame`, `split_frames`) |
| `commands` | command code enums + `StatusA1`, `RadarHardware`, `WorkingMode` |
| `data` | data frame header/parse + `Type4Record`, `Target`, `TrackFrame`, `CfarPoint`, `AlarmPoint` |
| `crc` | CRC‑16/MODBUS |
| `transport` | `UdpRadar` and `TcpRadar` clients |
| `error` | `Error` / `Result` |

## Data model (from the app's SQLite schemas)

* **frame** — `track_frame`: `version, frame_num, real_frame_num, refresh_time, boundary_a, boundary_b, direction`
* **target** — union of `track1/2/3_data`: `tid, frame_num, version, velocity, velocity_direction, distance, range, azimuth, height, snr, rcs, type, beam, doppler, dw, pulse, ri, di, priv_*`
* **cfar** — `cfar_data`: `distance, azimuth, height, pitch, doppler, di, ri, snr, beam, pulse`
* **alarm** — `alarm_data`: `frame_num, lat, lon, height, speed`

Timebase: each data frame carries an `i64` timestamp (header offset 8) and a frame
counter; one frame == one antenna revolution (sweep period from the working mode:
`Cir sweep 1s/2s/3s/4s`, `Fan sweep`, `Stand by`).

## Build & test

```sh
cargo build
cargo test
cargo run --example basic            # offline demo
cargo run --example basic 192.168.8.100:5002   # talk to a real radar
```

## Status / limitations

Confirmed from the binary: framing, checksums/CRC, command codes, the 16-byte header,
frame types, the type‑4 payload layout, and the full data model.

**Still to validate against a live capture** (see the `.md` docs shipped next to the app):
the semantic names of the `0xA1`/`0xA2` fields, the header bytes `b0`/`b1`/`b6`,
and the type‑3 / `0x22` record field maps. Names used here are documented as such.

## License

MIT OR Apache-2.0.
