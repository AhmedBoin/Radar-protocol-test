# fdad-radar — BWR‑T15 radar **TCP** protocol

Rust crate + Tk GUI + capture toolkit implementing the **FDAD‑DCT v3.0.0** radar interface.
Reverse‑engineered from `FDAD-DCTv3.0.0.exe` and **verified against a live BWR‑T15** at
`192.168.8.167:5001`.

* Full protocol spec: **[`PROTOCOL.md`](PROTOCOL.md)**
* Working notes: [`RADAR_PROTOCOL_TCP.md`](RADAR_PROTOCOL_TCP.md)
* *(the `RADAR_PROTOCOL_1/2/3_*.md` files describe an **older UDP** generation — NOT this radar; kept for reference.)*

## Quick facts
* **Transport:** TCP, client → `radar_ip:5001`, plaintext, big‑endian.
* **Frame:** `55 AA 55 AA | LEN(u32 BE = payload+2) | PAYLOAD | CRC16-MODBUS(payload) (u16 LE)`.
* **Login is required first** (`type 0x02`) or the radar streams nothing.
* **Control:** `SetRegister reg 0x401` = `0x00000401` → **rotate**, `0x00000000` → **stop**.

## RAW vs DERIVED (important)
The radar sends **Cartesian position + velocity**; all ranges/angles are computed by us.

### The radar actually sends (raw)
Per **data message** (`type 0x00030002`):
`MAC[6] · frameNum(u64) · timestamp(i64 ms) · boundaryA(u32) · boundaryB(u32) · count(u64)`,
then `count` × **64‑byte** records. Each record, raw:

| off | type | field | unit |
|---|---|---|---|
| 0 | i32 | X (east) | cm |
| 4 | i32 | Y (north) | cm |
| 8 | i32 | Height | cm |
| 12 | i32 | Vx | cm/s |
| 16 | i32 | Vy | cm/s |
| 20 | i32 | type / menace | — |
| 24 | u32 | ID (batch NO) | — |
| 28 | u32 | (SNR×100 << 16) \| (RCS×100) | — |

### We derive (NOT on the wire)
```
distance_m  = hypot(X, Y) / 100
azimuth_deg = degrees(atan2(X, Y)) mod 360     # 0° = north, clockwise
speed_mps   = hypot(Vx, Vy) / 100
heading_deg = degrees(atan2(Vx, Vy)) mod 360
sweep_deg   = boundary / 10000
height_m    = Height / 100
snr = (rec[28] >> 16)/100 ;  rcs = (rec[28] & 0xFFFF)/100
```
### The original app additionally derives (not sent)
`Pitch = atan2(Height, ground_range)`, and `Lon/Lat` by geo‑projecting (az, dist) from the
radar's configured site position.

## Layout
```
PROTOCOL.md                 full protocol spec
src/                        Rust crate: frame.rs, messages.rs, client.rs, crc.rs, error.rs
examples/radar_cli.rs       CLI: connect, login, receive+parse, control
gui/radar_gui.py            Tk GUI: Connect/Disconnect, Rotate/Stop, live stable vehicle plot
capture/                    Frida socket dumper, decoder, memory tool, pktmon, playbook
capture/samples/            a real captured frame sample
```

## Rust crate (complete)

```rust
use fdad_radar::{RadarClient, MessageType};

// connect() sends LOGIN automatically (required, or the radar streams nothing)
let mut c = RadarClient::connect("192.168.8.167:5001", None)?;

c.rotate()?;                                    // working-mode reg 0x401 = 0x401
// c.stop()?;                                   // 0x401 = 0
// c.set_register(&[(0x440, 0x0003_0102)])?;    // settings block
// c.set_json(r#"{"position":{"lat":29.8,"lon":31.08,"altitude":0.0,"yaw":0.0},"areas":[]}"#)?;
// c.get_register(&[0x440, 0x401])?;

// heartbeats (call ~every 10 s) + receive decoded target data
loop {
    for d in c.recv_data()? {                   // Vec<DataMessage>
        println!("sweep {:.1}..{:.1} deg, {} targets",
                 d.header.boundary_a_deg(), d.header.boundary_b_deg(), d.records.len());
        for t in &d.records {                   // t: fdad_radar::Record
            println!("  id={} dist={:.1}m az={:.1}deg h={:.1}m spd={:.1}m/s snr={:.1} rcs={:.2}",
                     t.id, t.distance_m(), t.azimuth_deg(), t.height_m(), t.speed_mps(), t.snr(), t.rcs());
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
}
```

### Public API

| Item | Purpose |
|---|---|
| `RadarClient::connect/login/heartbeat` | session (login is automatic on connect) |
| `RadarClient::rotate/stop` | working-mode control |
| `RadarClient::set_register/get_register/set_json/get_json` | config & registers |
| `RadarClient::recv` / `recv_data` | raw messages / decoded data messages |
| `Frame`, `FrameReader` | framing + TCP stream reassembly |
| `Message`, `MessageType` | `type | counter | body` |
| `DataHeader`, `Record`, `DataMessage` | decoded sweep header + target records |
| `messages::{login, heartbeat, set_register, get_register, set_json, get_json}` | payload builders |
| `messages::{REG_ROTATE, VAL_ROTATE, VAL_STOP, REG_WORKING_MODE, RECORD_LEN}` | constants |
| `crc::crc16_modbus` | CRC-16/MODBUS |

`Record` exposes raw fields (`x_cm, y_cm, height_cm, vx_cms, vy_cms, kind, id, packed`, plus the
full `raw: [u8; 64]`) and derived accessors (`distance_m, azimuth_deg, height_m, speed_mps,
heading_deg, snr, rcs`). Bytes `raw[32..64]` are retained but not yet semantically decoded.

### Frontend / Tauri v2 (`serde` feature)

Enable the optional `serde` feature to (de)serialize the types as **camelCase JSON**:

```toml
fdad-radar = { path = "…", features = ["serde"] }
```

Use the flat, render-ready `Sweep` / `Target` types. See **[`TAURI.md`](TAURI.md)** for a full
Tauri v2 backend (`radar_connect/rotate/stop/set_register` + `radar://sweep` event stream) and a
React/TS component, plus **[`frontend/radar.d.ts`](frontend/radar.d.ts)** for the hand-written
TypeScript types.

```rust
// in a Tauri command / event
let sweep: fdad_radar::Sweep = data_message.sweep();   // Serialize -> JS
```

## GUI
```sh
python gui/radar_gui.py     # set the IP/port, click Connect, then Rotate/Stop
```

## Build & test
```sh
cargo test        # frame + message encoders asserted byte-for-byte against real captures
cargo run --example radar_cli -- 192.168.8.167:5001 --secs 15
```

## License
MIT OR Apache-2.0.
