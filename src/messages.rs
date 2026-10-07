//! Message types, body codecs, and the data-stream header.

use crate::error::{Error, Result};

/// Message type codes (first `u32` of a payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// app→radar keep-alive (`0x00000001`).
    Heartbeat,
    /// app→radar set registers (`0x00000040`).
    SetRegister,
    /// app→radar set JSON settings (`0x00000080`).
    SetJson,
    /// app→radar get JSON (`0x00000082`).
    GetJson,
    /// radar→app data stream (`0x00030002`).
    Data,
    /// Anything else.
    Other(u32),
}

impl MessageType {
    /// Map a raw type code.
    pub fn from_u32(v: u32) -> Self {
        match v {
            0x0000_0001 => MessageType::Heartbeat,
            0x0000_0040 => MessageType::SetRegister,
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
            MessageType::SetRegister => 0x0000_0040,
            MessageType::SetJson => 0x0000_0080,
            MessageType::GetJson => 0x0000_0082,
            MessageType::Data => 0x0003_0002,
            MessageType::Other(o) => o,
        }
    }
}

/// A parsed message: `type` + `counter` + `body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Message type.
    pub mtype: MessageType,
    /// Outgoing/incoming monotonic counter.
    pub counter: u32,
    /// Body bytes after type+counter.
    pub body: Vec<u8>,
}

impl Message {
    /// Parse from a frame payload.
    pub fn parse(payload: &[u8]) -> Result<Self> {
        if payload.len() < 8 {
            return Err(Error::TruncatedPayload { need: 8, have: payload.len() });
        }
        let mtype = MessageType::from_u32(u32::from_be_bytes([
            payload[0], payload[1], payload[2], payload[3],
        ]));
        let counter = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        Ok(Message { mtype, counter, body: payload[8..].to_vec() })
    }
}

/// Build a heartbeat payload (`type | counter | u64(0)`).
pub fn heartbeat(counter: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(16);
    v.extend_from_slice(&0x0000_0001u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(&0u64.to_be_bytes());
    v
}

/// Build a SetRegister payload (`type | counter | count | (reg, value)*`).
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

/// Build a SetJson payload (`type | counter | len | 0 | json`).
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

/// Build a Login payload (`type=2 | counter | 00000003 00000000`).
/// MUST be the first message after connecting or the radar sends no data.
pub fn login(counter: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(16);
    v.extend_from_slice(&0x0000_0002u32.to_be_bytes());
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(&3u32.to_be_bytes());
    v.extend_from_slice(&0u32.to_be_bytes());
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

/// Data-stream header (radar→app), i.e. the body after the 8-byte message header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataHeader {
    /// Fixed device MAC / id (e.g. `37:9a:d3:9e:08:6e`).
    pub mac: [u8; 6],
    /// Message/sweep counter (+1 per message).
    pub frame_num: u64,
    /// Radar clock, milliseconds.
    pub timestamp: i64,
    /// Sector boundary A (`track_frame.boundaryA`).
    pub boundary_a: u32,
    /// Sector boundary B (`track_frame.boundaryB`).
    pub boundary_b: u32,
}

/// Parse the data body → `(header, record bytes)`.
///
/// Body layout: `u16 0x0006 | [6] mac | u64 frameNum | i64 ts | u32 bA | u32 bB | records`.
pub fn parse_data(body: &[u8]) -> Result<(DataHeader, &[u8])> {
    if body.len() < 32 {
        return Err(Error::TruncatedPayload { need: 32, have: body.len() });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn heartbeat_matches_capture() {
        let f = crate::frame::Frame::new(heartbeat(0x5e)).encode();
        assert_eq!(f, hex("55aa55aa00000012000000010000005e00000000000000000c12"));
    }

    #[test]
    fn setregister_matches_capture() {
        let m = set_register(142, &[(0x440, 0x0003_0102)]);
        assert_eq!(m, hex("000000400000008e000000010000044000030102"));
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
        let (h, _rec) = parse_data(&m.body).unwrap();
        assert_eq!(h.frame_num, 6678);
        assert_eq!(h.boundary_a, 3074242);
        assert_eq!(h.mac, [0x37, 0x9a, 0xd3, 0x9e, 0x08, 0x6e]);
    }
}
