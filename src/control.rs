//! Control channel — the `7E 7E …` framing (UDP).
//!
//! ```text
//! 7E 7E | LEN(u16 BE) | CMD(u8) | PAYLOAD | CHECKSUM(u8) | 0D 0A
//! ```
//! `LEN` is the payload length. `CHECKSUM` is the low byte of the sum of every
//! byte from offset 2 up to and including the last payload byte.

use crate::commands::{IncomingCommand, OutgoingCommand};
use crate::consts::{EOF, SOF};
use crate::error::{Error, Result};

/// Compute the control-frame checksum over `bytes` (which must start at frame offset 2).
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

/// One outgoing control frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFrame {
    /// Outgoing command.
    pub cmd: OutgoingCommand,
    /// Raw payload bytes.
    pub payload: Vec<u8>,
}

impl ControlFrame {
    /// Create a new frame.
    pub fn new(cmd: OutgoingCommand, payload: &[u8]) -> Self {
        ControlFrame { cmd, payload: payload.to_vec() }
    }

    /// Serialise to wire bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(8 + self.payload.len());
        v.push(SOF);
        v.push(SOF);
        v.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());
        v.push(self.cmd.wire_code());
        v.extend_from_slice(&self.payload);
        let sum = checksum(&v[2..]);
        v.push(sum);
        v.push(EOF[0]);
        v.push(EOF[1]);
        v
    }

    /// Parse an outgoing-style frame (CMD at offset 4). The CMD byte on the wire is
    /// the remapped value (`0x21`/`0x23`).
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let (cmd_byte, payload) = split(buf, 4)?;
        Ok(ControlFrame { cmd: OutgoingCommand::from_wire(cmd_byte)?, payload })
    }
}

/// A parsed incoming control frame (radar -> app; CMD at offset 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingFrame {
    /// Incoming command.
    pub cmd: IncomingCommand,
    /// Raw payload bytes.
    pub payload: Vec<u8>,
}

impl IncomingFrame {
    /// Parse an incoming frame. Note the CMD byte sits one byte later than on
    /// outgoing frames (uplink has an extra byte at offset 4).
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let (cmd_byte, payload) = split(buf, 5)?;
        Ok(IncomingFrame { cmd: IncomingCommand::from_wire(cmd_byte)?, payload })
    }
}

/// Shared validation + slice extraction.
///
/// `cmd_off` is 4 for outgoing frames, 5 for incoming frames.
fn split(buf: &[u8], cmd_off: usize) -> Result<(u8, Vec<u8>)> {
    if buf.len() < cmd_off + 4 {
        return Err(Error::FrameTooShort { len: buf.len(), min: cmd_off + 4 });
    }
    if buf[0] != SOF || buf[1] != SOF {
        return Err(Error::BadSof);
    }
    let len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    let need = cmd_off + 1 + len + 1 + EOF.len();
    if buf.len() < need {
        return Err(Error::FrameTooShort { len: buf.len(), min: need });
    }
    if buf[need - 2] != EOF[0] || buf[need - 1] != EOF[1] {
        return Err(Error::BadEof);
    }
    let computed = checksum(&buf[2..need - 3]);
    let found = buf[need - 3];
    if computed != found {
        return Err(Error::BadChecksum { expected: computed, found });
    }
    let cmd = buf[cmd_off];
    let payload = buf[cmd_off + 1..need - 3].to_vec();
    Ok((cmd, payload))
}

/// Split a byte stream into complete control frames.
///
/// Useful for a TCP/UDP stream where frames can be concatenated.
pub fn split_frames(buf: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 4 <= buf.len() {
        if buf[i] != SOF || buf[i + 1] != SOF {
            i += 1;
            continue;
        }
        let len = u16::from_be_bytes([buf[i + 2], buf[i + 3]]) as usize;
        // CMD(1) + payload(len) + checksum(1) + EOF(2), CMD at offset 4
        let total = 4 + 1 + len + 1 + EOF.len();
        if i + total <= buf.len() {
            out.push(&buf[i..i + total]);
            i += total;
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let f = ControlFrame::new(OutgoingCommand::Ctrl2A, &[1, 2, 3]);
        let bytes = f.encode();
        assert_eq!(bytes[0], 0x7E);
        assert_eq!(bytes[1], 0x7E);
        assert_eq!(bytes[4], 0x23); // wire code
        assert_eq!(&bytes[5..8], &[1, 2, 3]);
        assert_eq!(&bytes[bytes.len() - 2..], &[0x0D, 0x0A]);
        let back = ControlFrame::decode(&bytes).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn split_two() {
        let a = ControlFrame::new(OutgoingCommand::Ctrl1A, &[9]).encode();
        let b = ControlFrame::new(OutgoingCommand::Ctrl2A, &[]).encode();
        let mut all = a.clone();
        all.extend_from_slice(&b);
        let frames = split_frames(&all);
        assert_eq!(frames.len(), 2);
    }
}
