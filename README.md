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

## Rust crate
```rust
use fdad_radar::client::RadarClient;
use fdad_radar::messages::{self, MessageType};
// connect() logs in automatically
let mut c = RadarClient::connect("192.168.8.167:5001", None)?;
c.rotate()?;                 // start sweeping
c.stop()?;                   // stand by
for m in c.recv()? {
    if m.mtype == MessageType::Data {
        let (hdr, rec) = messages::parse_data(&m.body)?;
        println!("frame {} sweep {}..{}", hdr.frame_num, hdr.boundary_a / 10000, hdr.boundary_b / 10000);
    }
}
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
