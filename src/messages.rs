//! Message types, body codecs, the data-stream header, and target records.
//!
//! A message payload is `type(u32 BE) | counter(u32 BE) | body...`.
//! Verified against a live BWR-T15 at `192.168.8.167:5001`.

use crate::error::{Error, Result};

/// Message type codes (first `u32` of a payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum MessageType {
    /// app→radar keep-alive (`0x00000001`), sent ~every 10 s.
    Heartbeat,
    /// app→radar login (`0x00000002`). MUST be the first message or the radar streams nothing.
    Login,
    /// app→radar set registers (`0x00000040`).
    SetRegister,
    /// app→radar get registers (`0x00000041`).
    GetRegister,
    /// app→radar set JSON settings (`0x00000080`).
    SetJson,
    /// app→radar get JSON (`0x00000082`).
    GetJson,
    /// radar→app data stream (`0x00030002`).
    Data,
    /// Any other (forward-compatible).
    Other(u32),
}

impl MessageType {
    /// Map a raw type code.
    pub fn from_u32(v: u32) -> Self {
        match v {
            0x0000_0001 => MessageType::Heartbeat,
            0x0000_0002 => MessageType::Login,
            0x0000_0040 => MessageType::SetRegister,
            0x0000_0041 => MessageType::GetRegister,
            0x0000_0080 => MessageType::SetJson,
            0x0000_0082 => MessageType::GetJson,
            0x0003_0002 => MessageType::Data,
            o => MessageType::Other(o),
        }
    }
    /// Raw type code.
    pub fn as_u32(self) -> u32 {
        match self {
            MessageType::Heartbeat => 0x0000_0001,
            MessageType::Login => 0x0000_0002,
            MessageType::SetRegister => 0x0000_0040,
            MessageType::GetRegister => 0x0000_0041,
            MessageType::SetJson => 0x0000_0080,
            MessageType::GetJson => 0x0000_0082,
            MessageType::Data => 0x0003_0002,
            MessageType::Other(o) => o,
        }
    }
}

/// A parsed message: `type` + `counter` + `body`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Message {
    /// Message type.
    pub mtype: MessageType,
    /// Monotonic per-connection counter.
    pub counter: u32,
    /// Body bytes after type+counter.
    pub body: Vec<u8>,
}

impl Message {
    /// Parse from a frame payload (`type | counter | body`).
    pub fn parse(payload: &[u8]) -> Result<Self> {
        if payload.len() < 8 {
            return Err(Error::TruncatedPayload {
                need: 8,
                have: payload.len(),
            });
        }
        let mtype = MessageType::from_u32(u32::from_be_bytes([
            payload[0], payload[1], payload[2], payload[3],
        ]));
        let counter = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        Ok(Message {
            mtype,
            counter,
            body: payload[8..].to_vec(),
        })
    }
}

// ------------------------------- builders ---------------------------------

/// Build a heartbeat payload (`type=1 | counter | u64(0)`).
pub fn heartbeat(counter: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(16);
    v.extend_from_slice(&0x0000_0001u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(&0u64.to_be_bytes());
    v
}

/// Build a login payload (`type=2 | counter | 3 | 0`). Send once, first.
pub fn login(counter: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(16);
    v.extend_from_slice(&0x0000_0002u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(&3u32.to_be_bytes());
    v.extend_from_slice(&0u32.to_be_bytes());
    v
}

/// Build a SetRegister payload (`type=0x40 | counter | count | (reg,value)*`).
pub fn set_register(counter: u32, regs: &[(u32, u32)]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + 8 * regs.len());
    v.extend_from_slice(&0x0000_0040u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(&(regs.len() as u32).to_be_bytes());
    for (r, val) in regs {
        v.extend_from_slice(&r.to_be_bytes());
        v.extend_from_slice(&val.to_be_bytes());
    }
    v
}

/// Build a GetRegister payload (`type=0x41 | counter | count | reg*`).
pub fn get_register(counter: u32, regs: &[u32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + 4 * regs.len());
    v.extend_from_slice(&0x0000_0041u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(&(regs.len() as u32).to_be_bytes());
    for r in regs {
        v.extend_from_slice(&r.to_be_bytes());
    }
    v
}

/// Build a SetJson payload (`type=0x80 | counter | len | 0 | json`).
pub fn set_json(counter: u32, json: &str) -> Vec<u8> {
    let b = json.as_bytes();
    let mut v = Vec::with_capacity(16 + b.len());
    v.extend_from_slice(&0x0000_0080u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(&(b.len() as u32).to_be_bytes());
    v.extend_from_slice(&0u32.to_be_bytes());
    v.extend_from_slice(b);
    v
}

/// Build a GetJson payload (`type=0x82 | counter`).
pub fn get_json(counter: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(8);
    v.extend_from_slice(&0x0000_0082u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v
}

/// Register that controls rotation (`SetRegister`).
pub const REG_ROTATE: u32 = 0x0401;
/// Value that makes the radar **rotate** (Cir sweep).
pub const VAL_ROTATE: u32 = 0x0000_0401;
/// Value that makes the radar **stop** (Stand by).
pub const VAL_STOP: u32 = 0x0000_0000;
/// Register carrying the working-mode / settings block.
pub const REG_WORKING_MODE: u32 = 0x0440;

// --------------------------- data stream (radar→app) -----------------------

/// Data-stream header — the body after the 8-byte message header.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct DataHeader {
    /// Fixed device MAC / id (e.g. `37:9a:d3:9e:08:6e`).
    pub mac: [u8; 6],
    /// Message/sweep counter (+1 per message).
    pub frame_num: u64,
    /// Radar clock, milliseconds.
    pub timestamp: i64,
    /// Sector boundary A (start azimuth × 10000).
    pub boundary_a: u32,
    /// Sector boundary B (end azimuth × 10000).
    pub boundary_b: u32,
}

impl DataHeader {
    /// Sweep start azimuth in degrees.
    pub fn boundary_a_deg(&self) -> f64 {
        self.boundary_a as f64 / 10000.0
    }
    /// Sweep end azimuth in degrees.
    pub fn boundary_b_deg(&self) -> f64 {
        self.boundary_b as f64 / 10000.0
    }
}

/// Parse the data body → `(header, records_bytes)`.
///
/// Body: `u16 0x0006 | [6] mac | u64 frameNum | i64 ts | u32 bA | u32 bB | records`.
pub fn parse_data(body: &[u8]) -> Result<(DataHeader, &[u8])> {
    if body.len() < 32 {
        return Err(Error::TruncatedPayload {
            need: 32,
            have: body.len(),
        });
    }
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&body[2..8]);
    let h = DataHeader {
        mac,
        frame_num: u64::from_be_bytes(body[8..16].try_into().unwrap()),
        timestamp: i64::from_be_bytes(body[16..24].try_into().unwrap()),
        boundary_a: u32::from_be_bytes(body[24..28].try_into().unwrap()),
        boundary_b: u32::from_be_bytes(body[28..32].try_into().unwrap()),
    };
    Ok((h, &body[32..]))
}

/// Bytes per target record on the wire.
pub const RECORD_LEN: usize = 64;

/// One target/vehicle record (64 bytes). Raw values are big-endian.
///
/// Offsets: `X@0 Y@4 H@8 Vx@12 Vy@16 kind@20 id@24 packed@28` (i32/u32), then
/// 32 bytes (`raw[32..]`) that are **structured but not yet decoded**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Record {
    /// East position (cm).
    pub x_cm: i32,
    /// North position (cm).
    pub y_cm: i32,
    /// Height (cm).
    pub height_cm: i32,
    /// East velocity (cm/s).
    pub vx_cms: i32,
    /// North velocity (cm/s).
    pub vy_cms: i32,
    /// Type / menace field (offset 20) — semantics not yet confirmed.
    pub kind: i32,
    /// Track id / batch NO (offset 24).
    pub id: u32,
    /// Packed `(snr*100 << 16) | (rcs*100)` (offset 28).
    pub packed: u32,
    /// The full raw 64 bytes (tail `[32..64]` reserved for future fields).
    /// Skipped by serde to keep JSON compact; use [`Record::target`] for a flat view.
    #[cfg_attr(feature = "serde", serde(skip, default = "raw_default"))]
    pub raw: [u8; RECORD_LEN],
}

impl Record {
    /// Decode one 64-byte record.
    pub fn decode(r: &[u8]) -> Result<Self> {
        if r.len() < RECORD_LEN {
            return Err(Error::TruncatedPayload {
                need: RECORD_LEN,
                have: r.len(),
            });
        }
        let g = |o: usize| i32::from_be_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
        let u = |o: usize| u32::from_be_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
        let mut raw = [0u8; RECORD_LEN];
        raw.copy_from_slice(&r[..RECORD_LEN]);
        Ok(Record {
            x_cm: g(0),
            y_cm: g(4),
            height_cm: g(8),
            vx_cms: g(12),
            vy_cms: g(16),
            kind: g(20),
            id: u(24),
            packed: u(28),
            raw,
        })
    }
    /// Ground distance (m) = `hypot(X, Y) / 100`.
    pub fn distance_m(&self) -> f64 {
        (self.x_cm as f64).hypot(self.y_cm as f64) / 100.0
    }
    /// Azimuth (deg), 0 = north, clockwise = `atan2(X, Y)` mod 360.
    pub fn azimuth_deg(&self) -> f64 {
        (self.x_cm as f64)
            .atan2(self.y_cm as f64)
            .to_degrees()
            .rem_euclid(360.0)
    }
    /// Height (m).
    pub fn height_m(&self) -> f64 {
        self.height_cm as f64 / 100.0
    }
    /// Ground speed (m/s).
    pub fn speed_mps(&self) -> f64 {
        (self.vx_cms as f64).hypot(self.vy_cms as f64) / 100.0
    }
    /// Heading (deg), 0 = north = `atan2(Vx, Vy)` mod 360.
    pub fn heading_deg(&self) -> f64 {
        (self.vx_cms as f64)
            .atan2(self.vy_cms as f64)
            .to_degrees()
            .rem_euclid(360.0)
    }
    /// SNR (÷100 of the high u16).
    pub fn snr(&self) -> f64 {
        (self.packed >> 16) as f64 / 100.0
    }
    /// RCS (÷100 of the low u16).
    pub fn rcs(&self) -> f64 {
        (self.packed & 0xFFFF) as f64 / 100.0
    }
}

/// Parse the records area: `u64 count | count × 64-byte records`.
///
/// Returns an empty vec if `count` is 0 or implausible (mirrors the reference impl).
pub fn parse_records(rec: &[u8]) -> Result<Vec<Record>> {
    if rec.len() < 8 {
        return Ok(Vec::new());
    }
    let n = u64::from_be_bytes(rec[..8].try_into().unwrap()) as usize;
    if n == 0 || n > 4096 || 8 + n * RECORD_LEN > rec.len() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let off = 8 + k * RECORD_LEN;
        out.push(Record::decode(&rec[off..off + RECORD_LEN])?);
    }
    Ok(out)
}

/// A fully decoded data message: sweep header + target records.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct DataMessage {
    /// Sweep header.
    pub header: DataHeader,
    /// Target records.
    pub records: Vec<Record>,
}

/// Convenience: parse a data-message body into header + records.
pub fn parse_data_message(body: &[u8]) -> Result<DataMessage> {
    let (header, rec) = parse_data(body)?;
    Ok(DataMessage {
        header,
        records: parse_records(rec)?,
    })
}

/// Zero default for [`Record::raw`] (used by `serde(skip)`).
#[cfg(feature = "serde")]
fn raw_default() -> [u8; RECORD_LEN] {
    [0u8; RECORD_LEN]
}

/// Build a DATA message payload (radar→app) for one sweep chunk.
///
/// Layout: `type 0x00030002 | counter | u16 6 | mac[6] | frameNum | ts |
/// boundary_a | boundary_b | u64 count | count × 64-byte records`.
#[allow(clippy::too_many_arguments)]
pub fn data_message(
    counter: u32,
    mac: [u8; 6],
    frame_num: u64,
    timestamp: i64,
    boundary_a: u32,
    boundary_b: u32,
    records: &[Record],
) -> Vec<u8> {
    let mut v = Vec::with_capacity(48 + records.len() * RECORD_LEN);
    v.extend_from_slice(&0x0003_0002u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(&6u16.to_be_bytes());
    v.extend_from_slice(&mac);
    v.extend_from_slice(&frame_num.to_be_bytes());
    v.extend_from_slice(&timestamp.to_be_bytes());
    v.extend_from_slice(&boundary_a.to_be_bytes());
    v.extend_from_slice(&boundary_b.to_be_bytes());
    v.extend_from_slice(&(records.len() as u64).to_be_bytes());
    for r in records {
        v.extend_from_slice(&r.encode());
    }
    v
}

/// A flat, frontend-ready target: the raw record **plus** derived values.
///
/// Return this from a Tauri v2 command to a React/TypeScript frontend.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Target {
    /// Track id.
    pub id: u32,
    /// East position (cm).
    pub x_cm: i32,
    /// North position (cm).
    pub y_cm: i32,
    /// Height (cm).
    pub height_cm: i32,
    /// East velocity (cm/s).
    pub vx_cms: i32,
    /// North velocity (cm/s).
    pub vy_cms: i32,
    /// Type / menace.
    pub kind: i32,
    /// Ground distance (m).
    pub distance_m: f64,
    /// Azimuth (deg).
    pub azimuth_deg: f64,
    /// Height (m).
    pub height_m: f64,
    /// Ground speed (m/s).
    pub speed_mps: f64,
    /// Heading (deg).
    pub heading_deg: f64,
    /// SNR.
    pub snr: f64,
    /// RCS.
    pub rcs: f64,
}

impl Record {
    /// Flatten into a [`Target`] with derived values, ready for the frontend.
    pub fn target(&self) -> Target {
        Target {
            id: self.id,
            x_cm: self.x_cm,
            y_cm: self.y_cm,
            height_cm: self.height_cm,
            vx_cms: self.vx_cms,
            vy_cms: self.vy_cms,
            kind: self.kind,
            distance_m: self.distance_m(),
            azimuth_deg: self.azimuth_deg(),
            height_m: self.height_m(),
            speed_mps: self.speed_mps(),
            heading_deg: self.heading_deg(),
            snr: self.snr(),
            rcs: self.rcs(),
        }
    }

    /// Construct a record from fields (the 32-byte tail is zeroed).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        x_cm: i32,
        y_cm: i32,
        height_cm: i32,
        vx_cms: i32,
        vy_cms: i32,
        kind: i32,
        id: u32,
        packed: u32,
    ) -> Self {
        Record {
            x_cm,
            y_cm,
            height_cm,
            vx_cms,
            vy_cms,
            kind,
            id,
            packed,
            raw: [0u8; RECORD_LEN],
        }
    }

    /// Encode to the 64-byte wire form (big-endian fields + preserved `raw` tail).
    pub fn encode(&self) -> [u8; RECORD_LEN] {
        let mut r = self.raw;
        r[0..4].copy_from_slice(&self.x_cm.to_be_bytes());
        r[4..8].copy_from_slice(&self.y_cm.to_be_bytes());
        r[8..12].copy_from_slice(&self.height_cm.to_be_bytes());
        r[12..16].copy_from_slice(&self.vx_cms.to_be_bytes());
        r[16..20].copy_from_slice(&self.vy_cms.to_be_bytes());
        r[20..24].copy_from_slice(&self.kind.to_be_bytes());
        r[24..28].copy_from_slice(&self.id.to_be_bytes());
        r[28..32].copy_from_slice(&self.packed.to_be_bytes());
        r
    }
}

/// A flat, frontend-ready sweep: header summary + all targets.
///
/// This is the natural return type for a Tauri v2 command feeding a radar view.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Sweep {
    /// Device MAC (6 bytes).
    pub mac: [u8; 6],
    /// Sweep/message counter.
    pub frame_num: u64,
    /// Radar clock (ms).
    pub timestamp: i64,
    /// Sweep start azimuth (deg).
    pub boundary_a_deg: f64,
    /// Sweep end azimuth (deg).
    pub boundary_b_deg: f64,
    /// Number of targets in this sweep.
    pub target_count: usize,
    /// Targets (flat, with derived values).
    pub targets: Vec<Target>,
}

impl DataMessage {
    /// Convert to a flat, serializable [`Sweep`] for the frontend.
    pub fn sweep(&self) -> Sweep {
        Sweep {
            mac: self.header.mac,
            frame_num: self.header.frame_num,
            timestamp: self.header.timestamp,
            boundary_a_deg: self.header.boundary_a_deg(),
            boundary_b_deg: self.header.boundary_b_deg(),
            target_count: self.records.len(),
            targets: self.records.iter().map(|r| r.target()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn builders_match_capture() {
        assert_eq!(login(0), hex("00000002000000000000000300000000"));
        assert_eq!(
            crate::frame::Frame::new(heartbeat(0x5e)).encode(),
            hex("55aa55aa00000012000000010000005e00000000000000000c12")
        );
        assert_eq!(
            get_register(1, &[0x440, 0x401]),
            hex("0000004100000001000000020000044000000401")
        );
    }

    /// A real 64-byte record from `capture/samples/radar-live-sample.tsv` (16 big-endian dwords).
    const LIVE_RECORD: [u32; 16] = [
        0x00025f88, 0xfff9a727, 0x0001669f, 0xfffffe79, 0xfffffd98, 0xffffffe5, 0x00001735,
        0x00000000, 0xffffffff, 0xffff0000, 0x00000197, 0x739a605a, 0x00010000, 0x030c0000,
        0x00000000, 0x00000000,
    ];

    fn live_record_bytes() -> Vec<u8> {
        let mut v = Vec::with_capacity(64);
        for d in LIVE_RECORD {
            v.extend_from_slice(&d.to_be_bytes());
        }
        v
    }

    #[test]
    fn record_from_live_capture() {
        let rec = Record::decode(&live_record_bytes()).unwrap();
        assert_eq!(rec.x_cm, 155528);
        assert_eq!(rec.y_cm, -415961);
        assert_eq!(rec.height_cm, 91807);
        assert_eq!(rec.id, 5941);
        assert!((rec.distance_m() - 4440.9).abs() < 3.0);
        assert!((rec.height_m() - 918.07).abs() < 0.01);
    }

    #[test]
    fn records_area_parses() {
        let mut area = 1u64.to_be_bytes().to_vec(); // count = 1
        area.extend_from_slice(&live_record_bytes());
        let recs = parse_records(&area).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].id, 5941);
    }

    #[test]
    fn data_header_parses() {
        let mut p = vec![0x00, 0x03, 0x00, 0x02];
        p.extend_from_slice(&0x5e5eu32.to_be_bytes());
        p.extend_from_slice(&[0x00, 0x06, 0x37, 0x9a, 0xd3, 0x9e, 0x08, 0x6e]);
        p.extend_from_slice(&6678u64.to_be_bytes());
        p.extend_from_slice(&1749988604581i64.to_be_bytes());
        p.extend_from_slice(&3074242u32.to_be_bytes());
        p.extend_from_slice(&3404819u32.to_be_bytes());
        let m = Message::parse(&p).unwrap();
        assert_eq!(m.mtype, MessageType::Data);
        let (h, _) = parse_data(&m.body).unwrap();
        assert_eq!(h.frame_num, 6678);
        assert_eq!(h.mac, [0x37, 0x9a, 0xd3, 0x9e, 0x08, 0x6e]);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serializes_for_frontend() {
        let t = Record::decode(&live_record_bytes()).unwrap().target();
        let v = serde_json::to_value(t).unwrap();
        assert_eq!(v["id"], serde_json::json!(5941));
        assert!(v.get("distanceM").is_some(), "expected camelCase distanceM");
        assert!(v.get("raw").is_none(), "raw must be skipped");

        let mut body = vec![0x00, 0x03, 0x00, 0x02];
        body.extend_from_slice(&7u32.to_be_bytes());
        body.extend_from_slice(&[0x00, 0x06, 0x37, 0x9a, 0xd3, 0x9e, 0x08, 0x6e]);
        body.extend_from_slice(&42u64.to_be_bytes());
        body.extend_from_slice(&1749988604581i64.to_be_bytes());
        body.extend_from_slice(&3074242u32.to_be_bytes());
        body.extend_from_slice(&3404819u32.to_be_bytes());
        body.extend_from_slice(&1u64.to_be_bytes());
        body.extend_from_slice(&live_record_bytes());

        let m = Message::parse(&body).unwrap();
        let dm = parse_data_message(&m.body).unwrap();
        let s = serde_json::to_string(&dm.sweep()).unwrap();
        assert!(s.contains("\"targetCount\":1"), "{s}");
        assert!(s.contains("\"frameNum\":42"), "{s}");
        let back: Sweep = serde_json::from_str(&s).unwrap();
        assert_eq!(back.targets.len(), 1);
    }

    #[test]
    fn data_message_roundtrip() {
        let recs = vec![Record::new(
            155528, -415961, 91807, -391, -616, -27, 5941, 0,
        )];
        let payload = data_message(
            7,
            [0x37, 0x9a, 0xd3, 0x9e, 0x08, 0x6e],
            42,
            1749988604581,
            3074242,
            3404819,
            &recs,
        );
        assert_eq!(payload.len(), 48 + RECORD_LEN);

        let wire = crate::frame::Frame::new(payload.clone()).encode();
        let f = crate::frame::Frame::decode(&wire).unwrap();
        assert_eq!(f.payload, payload);

        let m = Message::parse(&f.payload).unwrap();
        assert_eq!(m.mtype, MessageType::Data);
        let dm = parse_data_message(&m.body).unwrap();
        assert_eq!(dm.header.frame_num, 42);
        assert_eq!(dm.records.len(), 1);
        assert_eq!(dm.records[0].id, 5941);
        assert_eq!(dm.records[0].x_cm, 155528);
    }
}
