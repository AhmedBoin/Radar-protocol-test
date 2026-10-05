//! Data channel — 16-byte header + payload + CRC-16/MODBUS.
//!
//! ```text
//! off  size  field
//! 0    1     b0        u8
//! 1    1     b1        u8
//! 2    2     TYPE      u16 BE
//! 4    2     LENGTH    u16 BE   (total frame length)
//! 6    2     b6        u16 BE
//! 8    8     TIMESTAMP i64 BE
//! 16   LEN-18 PAYLOAD
//! LEN-2 2    CRC16             (CRC-16/MODBUS over bytes[0..LEN-2])
//! ```

use crate::consts::{DATA_CRC_LEN, DATA_HEADER_LEN, DATA_MIN_LEN};
use crate::crc::crc16_modbus;
use crate::error::{Error, Result};

/// Known data-frame types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// Type 3 — 52-byte record.
    Type3,
    /// Type 4 — 64-byte record (payload layout decoded).
    Type4,
    /// Type 0x22 — list of records.
    Type22,
    /// Any other value seen on the wire.
    Other(u16),
}

impl FrameType {
    /// Map a raw type code.
    pub fn from_u16(v: u16) -> Self {
        match v {
            3 => FrameType::Type3,
            4 => FrameType::Type4,
            0x22 => FrameType::Type22,
            other => FrameType::Other(other),
        }
    }

    /// Raw code.
    pub fn as_u16(self) -> u16 {
        match self {
            FrameType::Type3 => 3,
            FrameType::Type4 => 4,
            FrameType::Type22 => 0x22,
            FrameType::Other(v) => v,
        }
    }
}

/// Decoded fixed frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataHeader {
    /// Byte 0 — unit/address (meaning TBD).
    pub b0: u8,
    /// Byte 1 — flags (meaning TBD).
    pub b1: u8,
    /// Message type.
    pub frame_type: u16,
    /// Total frame length (header + payload + crc).
    pub length: u16,
    /// Bytes 6..8 (counter, meaning TBD).
    pub b6: u16,
    /// Frame timestamp (time base).
    pub timestamp: i64,
}

impl DataHeader {
    /// The typed frame type.
    pub fn kind(&self) -> FrameType {
        FrameType::from_u16(self.frame_type)
    }

    /// Payload length implied by `length`.
    pub fn payload_len(&self) -> Option<usize> {
        (self.length as usize).checked_sub(DATA_HEADER_LEN + DATA_CRC_LEN)
    }
}

/// A validated data frame (header + payload, CRC checked).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFrame {
    /// Header.
    pub header: DataHeader,
    /// Payload bytes (without header or CRC).
    pub payload: Vec<u8>,
}

impl DataFrame {
    /// Validate and parse a complete frame (as received from the socket).
    pub fn parse(buf: &[u8]) -> Result<Self> {
        if buf.len() < DATA_MIN_LEN {
            return Err(Error::FrameTooShort { len: buf.len(), min: DATA_MIN_LEN });
        }
        let length = u16::from_be_bytes([buf[4], buf[5]]) as usize;
        if length < DATA_MIN_LEN || length > buf.len() {
            return Err(Error::FrameTooShort { len: buf.len(), min: length });
        }
        let stored = u16::from_le_bytes([buf[length - 2], buf[length - 1]]);
        let computed = crc16_modbus(&buf[..length - DATA_CRC_LEN]);
        if computed != stored {
            return Err(Error::BadCrc { expected: computed, found: stored });
        }
        let header = DataHeader {
            b0: buf[0],
            b1: buf[1],
            frame_type: u16::from_be_bytes([buf[2], buf[3]]),
            length: length as u16,
            b6: u16::from_be_bytes([buf[6], buf[7]]),
            timestamp: i64::from_be_bytes([
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]),
        };
        let payload = buf[DATA_HEADER_LEN..length - DATA_CRC_LEN].to_vec();
        Ok(DataFrame { header, payload })
    }

    /// Build a frame from a header + payload (CRC appended automatically).
    pub fn build(b0: u8, b1: u8, frame_type: u16, b6: u16, timestamp: i64, payload: &[u8]) -> Vec<u8> {
        let length = (DATA_HEADER_LEN + payload.len() + DATA_CRC_LEN) as u16;
        let mut v = Vec::with_capacity(length as usize);
        v.push(b0);
        v.push(b1);
        v.extend_from_slice(&frame_type.to_be_bytes());
        v.extend_from_slice(&length.to_be_bytes());
        v.extend_from_slice(&b6.to_be_bytes());
        v.extend_from_slice(&timestamp.to_be_bytes());
        v.extend_from_slice(payload);
        let crc = crc16_modbus(&v);
        v.extend_from_slice(&crc.to_le_bytes());
        v
    }
}

/// Minimal big-endian cursor for reading fixed record payloads.
struct Cursor<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(b: &'a [u8]) -> Self {
        Cursor { b, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.b.len() {
            return Err(Error::TruncatedPayload { need: self.pos + n, have: self.b.len() });
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    fn i32(&mut self) -> Result<i32> {
        let s = self.take(4)?;
        Ok(i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f64(&mut self) -> Result<f64> {
        let s = self.take(8)?;
        Ok(f64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }
    fn bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        Ok(self.take(n)?.to_vec())
    }
}

/// Payload of a **type-4** frame (layout confirmed from the binary).
///
/// Field types are exact; the letter names are placeholders until a live capture
/// binds them to their real meaning (position/kinematics/etc).
#[derive(Debug, Clone, PartialEq)]
pub struct Type4Record {
    /// 64-byte id/name buffer (NUL padded).
    pub id: Vec<u8>,
    /// i32 @ +0x40
    pub a: i32,
    /// i32 @ +0x44
    pub b: i32,
    /// f64 @ +0x48
    pub c: f64,
    /// f64 @ +0x50
    pub d: f64,
    /// f64 @ +0x58
    pub e: f64,
    /// i32 @ +0x60
    pub f: i32,
    /// f64 @ +0x68
    pub g: f64,
    /// i8  @ +0x70
    pub h: i8,
    /// f64 @ +0x78
    pub i: f64,
    /// f64 @ +0x80
    pub j: f64,
    /// i8  @ +0x88
    pub k: i8,
    /// i8  @ +0x89
    pub l: i8,
    /// 16 bytes @ +0x8A
    pub m: Vec<u8>,
    /// 16 bytes @ +0x9A
    pub n: Vec<u8>,
}

impl Type4Record {
    /// Decode a type-4 payload.
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let mut c = Cursor::new(payload);
        Ok(Type4Record {
            id: c.bytes(64)?,
            a: c.i32()?,
            b: c.i32()?,
            c: c.f64()?,
            d: c.f64()?,
            e: c.f64()?,
            f: c.i32()?,
            g: c.f64()?,
            h: c.i8()?,
            i: c.f64()?,
            j: c.f64()?,
            k: c.i8()?,
            l: c.i8()?,
            m: c.bytes(16)?,
            n: c.bytes(16)?,
        })
    }

    /// The id/name as a lossy UTF-8 string.
    pub fn id_str(&self) -> std::borrow::Cow<'_, str> {
        let end = self.id.iter().position(|&x| x == 0).unwrap_or(self.id.len());
        String::from_utf8_lossy(&self.id[..end])
    }
}

/// Target classification (`type` field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetType {
    /// Not yet detected / unknown.
    #[default]
    UnDetected,
    /// Multiple birds.
    MultiBirds,
    /// Airplane.
    Airplane,
    /// Drone / UAV.
    Drone,
    /// Bird.
    Bird,
    /// Pedestrian.
    Pedestrian,
    /// Vehicle.
    Vehicle,
    /// Unknown.
    Unknown,
    /// Any other raw value.
    Other(i32),
}

impl TargetType {
    /// Map a raw `type` value (mapping inferred from the UI strings).
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => TargetType::UnDetected,
            1 => TargetType::MultiBirds,
            2 => TargetType::Airplane,
            3 => TargetType::Drone,
            4 => TargetType::Bird,
            5 => TargetType::Pedestrian,
            6 => TargetType::Vehicle,
            7 => TargetType::Unknown,
            other => TargetType::Other(other),
        }
    }
}

/// One radar **target/track** (union of the `track1/2/3_data` schemas).
///
/// Not every field is present in every variant — the radar fills what it reports.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Target {
    /// Track id (i64) — stable across frames.
    pub tid: i64,
    /// Frame index this record belongs to.
    pub frame_num: i32,
    /// Data version.
    pub version: i32,
    /// Speed (m/s).
    pub velocity: f64,
    /// Direction of the velocity vector (deg).
    pub velocity_direction: f64,
    /// Slant distance (m).
    pub distance: f64,
    /// Ground range (m).
    pub range: f64,
    /// Azimuth (deg).
    pub azimuth: f64,
    /// Height (m).
    pub height: f64,
    /// Signal-to-noise ratio (dB).
    pub snr: f64,
    /// Radar cross section (m^2).
    pub rcs: f64,
    /// Classification.
    pub target_type: TargetType,
    /// Beam index.
    pub beam: i32,
    /// Doppler bin.
    pub doppler: i32,
    /// `dw` descriptor.
    pub dw: i32,
    /// Pulse-width index.
    pub pulse: i32,
    /// Range index (CFAR).
    pub ri: i32,
    /// Doppler index (CFAR).
    pub di: i32,
    /// Previous-frame distance (for trails).
    pub priv_distance: f64,
    /// Previous-frame azimuth.
    pub priv_azimuth: f64,
    /// Previous-frame velocity.
    pub priv_velocity: f64,
    /// Previous-frame velocity direction.
    pub priv_velocity_direction: f64,
    /// Previous-frame height.
    pub priv_height: f64,
}

/// One radar **sweep/frame** (`track_frame`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TrackFrame {
    /// Data version.
    pub version: i32,
    /// Sweep index.
    pub frame_num: i32,
    /// Raw radar frame index.
    pub real_frame_num: i32,
    /// Wall-clock time of the frame.
    pub refresh_time: i64,
    /// Detection sector boundary A.
    pub boundary_a: f64,
    /// Detection sector boundary B.
    pub boundary_b: f64,
    /// Antenna/boresight direction.
    pub direction: i32,
}

/// One CFAR plot (`cfar_data`) — a raw detection, not a track.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CfarPoint {
    /// Distance (m).
    pub distance: f64,
    /// Azimuth (deg).
    pub azimuth: f64,
    /// Height (m).
    pub height: f64,
    /// Pitch (deg).
    pub pitch: f64,
    /// Doppler bin.
    pub doppler: i32,
    /// Doppler index.
    pub di: i32,
    /// Range index.
    pub ri: i32,
    /// SNR (dB).
    pub snr: f64,
    /// Beam index.
    pub beam: i32,
    /// Pulse index.
    pub pulse: i32,
}

/// Geo-referenced alarm record (`alarm_data`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AlarmPoint {
    /// Frame index.
    pub frame_num: i32,
    /// Latitude (deg).
    pub lat: f64,
    /// Longitude (deg).
    pub lon: f64,
    /// Height (m).
    pub height: f64,
    /// Speed (m/s).
    pub speed: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let payload = vec![1u8, 2, 3, 4, 5];
        let bytes = DataFrame::build(0x11, 0x22, 4, 7, 1234567890, &payload);
        let f = DataFrame::parse(&bytes).unwrap();
        assert_eq!(f.header.b0, 0x11);
        assert_eq!(f.header.b1, 0x22);
        assert_eq!(f.header.kind(), FrameType::Type4);
        assert_eq!(f.header.timestamp, 1234567890);
        assert_eq!(f.payload, payload);
    }

    #[test]
    fn bad_crc_detected() {
        let mut bytes = DataFrame::build(0, 0, 3, 0, 0, &[9, 9, 9]);
        let n = bytes.len();
        bytes[n - 1] ^= 0xFF;
        assert!(matches!(DataFrame::parse(&bytes), Err(Error::BadCrc { .. })));
    }

    #[test]
    fn type4_decode() {
        // build a valid type-4 payload
        let mut p = vec![0u8; 64];
        p[0..4].copy_from_slice(b"test");
        p.extend_from_slice(&1i32.to_be_bytes());
        p.extend_from_slice(&2i32.to_be_bytes());
        p.extend_from_slice(&3.5f64.to_be_bytes());
        p.extend_from_slice(&4.5f64.to_be_bytes());
        p.extend_from_slice(&5.5f64.to_be_bytes());
        p.extend_from_slice(&6i32.to_be_bytes());
        p.extend_from_slice(&7.5f64.to_be_bytes());
        p.push(8);
        p.extend_from_slice(&9.5f64.to_be_bytes());
        p.extend_from_slice(&10.5f64.to_be_bytes());
        p.push(11);
        p.push(12);
        p.extend_from_slice(&[0u8; 16]);
        p.extend_from_slice(&[0u8; 16]);
        let r = Type4Record::decode(&p).unwrap();
        assert_eq!(r.id_str(), "test");
        assert_eq!(r.a, 1);
        assert_eq!(r.g, 7.5);
        assert_eq!(r.h, 8);
        assert_eq!(r.k, 11);
    }
}
