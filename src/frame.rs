//! TCP frame codec.
//!
//! Wire format (both directions, all multi-byte ints big-endian):
//! ```text
//! 55 AA 55 AA | LEN (u32 BE = payload+2) | PAYLOAD (LEN-2) | CRC16-MODBUS(payload) (u16 LE)
//! total bytes = LEN + 8
//! ```

use crate::crc::crc16_modbus;
use crate::error::{Error, Result};

/// Frame magic `55 AA 55 AA`.
pub const MAGIC: u32 = 0x55AA_55AA;
/// Framing bytes before the payload (magic + length field).
pub const HEADER_LEN: usize = 8;
/// Size of the trailing CRC.
pub const CRC_LEN: usize = 2;

/// One protocol frame (payload only; framing is added/removed here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Message payload `type(u32) | counter(u32) | body...`, without magic/length/CRC.
    pub payload: Vec<u8>,
}

impl Frame {
    /// Wrap a payload into a frame.
    pub fn new(payload: Vec<u8>) -> Self {
        Frame { payload }
    }

    /// Encode to wire bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(HEADER_LEN + self.payload.len() + CRC_LEN);
        v.extend_from_slice(&MAGIC.to_be_bytes());
        v.extend_from_slice(&((self.payload.len() + CRC_LEN) as u32).to_be_bytes());
        v.extend_from_slice(&self.payload);
        v.extend_from_slice(&crc16_modbus(&self.payload).to_le_bytes());
        v
    }

    /// Decode a complete frame (exactly one whole frame, CRC-checked).
    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < HEADER_LEN {
            return Err(Error::FrameTooShort {
                len: buf.len(),
                min: HEADER_LEN,
            });
        }
        let magic = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if magic != MAGIC {
            return Err(Error::BadMagic(magic));
        }
        let len = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
        if len < CRC_LEN {
            return Err(Error::FrameTooShort { len, min: CRC_LEN });
        }
        let total = HEADER_LEN + len;
        if buf.len() < total {
            return Err(Error::FrameTooShort {
                len: buf.len(),
                min: total,
            });
        }
        let pl_end = HEADER_LEN + len - CRC_LEN;
        let payload = buf[HEADER_LEN..pl_end].to_vec();
        let stored = u16::from_le_bytes([buf[pl_end], buf[pl_end + 1]]);
        let crc = crc16_modbus(&payload);
        if crc != stored {
            return Err(Error::BadCrc {
                expected: crc,
                found: stored,
            });
        }
        Ok(Frame { payload })
    }
}

/// A TCP stream reassembler: feed bytes, drain whole CRC-validated frames.
#[derive(Default)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    /// New empty reader.
    pub fn new() -> Self {
        FrameReader { buf: Vec::new() }
    }

    /// Feed bytes and drain any complete frames.
    pub fn push(&mut self, data: &[u8]) -> Vec<Frame> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            let Some(p) = self.buf.windows(4).position(|w| w == MAGIC.to_be_bytes()) else {
                // No magic yet: keep the last 3 bytes, which may be a partial magic.
                let keep = self.buf.len().saturating_sub(3);
                if keep > 0 {
                    self.buf.drain(0..keep);
                }
                break;
            };
            if p > 0 {
                self.buf.drain(0..p);
            }
            if self.buf.len() < HEADER_LEN {
                break;
            }
            let len =
                u32::from_be_bytes([self.buf[4], self.buf[5], self.buf[6], self.buf[7]]) as usize;
            let total = HEADER_LEN + len;
            if self.buf.len() < total {
                break;
            }
            let chunk: Vec<u8> = self.buf[..total].to_vec();
            if let Ok(f) = Frame::decode(&chunk) {
                out.push(f);
            }
            self.buf.drain(0..total);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let f = Frame::new(vec![
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x5e, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        let b = f.encode();
        assert_eq!(&b[0..4], &[0x55, 0xAA, 0x55, 0xAA]);
        // heartbeat from the live capture ends with CRC 0x120C -> 0C 12
        assert_eq!(
            &b[..26],
            &hex("55aa55aa00000012000000010000005e00000000000000000c12")[..]
        );
        assert_eq!(Frame::decode(&b).unwrap(), f);
    }

    #[test]
    fn reader_splits() {
        let a = Frame::new(vec![1, 2, 3, 4]).encode();
        let b = Frame::new(vec![9, 9]).encode();
        let mut stream = a.clone();
        stream.extend_from_slice(&b);
        let mut r = FrameReader::new();
        // feed in awkward chunks
        let mut got = r.push(&stream[..3]);
        got.extend(r.push(&stream[3..]));
        assert_eq!(got.len(), 2);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
