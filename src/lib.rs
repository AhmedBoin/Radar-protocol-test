//! # fdad-radar
//!
//! A Rust library for the **FDAD-DCT v3.0.0** radar interface protocol.
//!
//! Reverse-engineered from `FDAD-DCTv3.0.0.exe`. It gives you everything needed to
//! build a new application that talks to the radar: build/parse the control channel,
//! validate and decode data frames, and read typed target/sweep data.
//!
//! ## Two channels
//!
//! ### 1. Control channel (UDP) — [`control`]
//! Framing (`QDataStream` big-endian):
//! ```text
//! 7E 7E | LEN(u16 BE) | CMD(u8) | PAYLOAD | CHECKSUM(u8) | 0D 0A
//! ```
//! `CHECKSUM = (sum of bytes from offset 2 .. end of payload) & 0xFF`.
//!
//! * Outgoing CMD: internal `0x1A` -> wire `0x21`, internal `0x2A` -> wire `0x23`.
//! * Incoming CMD: `0xA1`, `0xA2`.
//!
//! ### 2. Data channel (UDP) — [`data`]
//! 16-byte header + payload + **CRC-16/MODBUS**:
//! ```text
//! 0: u8 b0 | 1: u8 b1 | 2: u16 TYPE | 4: u16 LEN | 6: u16 b6 | 8: i64 TIMESTAMP
//! 16: payload (LEN-18 bytes) | LEN-2: u16 CRC16  (CRC over bytes[0..LEN-2])
//! ```
//! Frame types: `3`, `4`, `0x22`.
//!
//! ## Example
//! ```
//! use fdad_radar::commands::OutgoingCommand;
//! use fdad_radar::control::ControlFrame;
//!
//! // Build a control frame the radar understands
//! let f = ControlFrame::new(OutgoingCommand::Ctrl2A, &[0x00, 0x01, 0x02]);
//! let bytes = f.encode();
//! assert_eq!(&bytes[0..2], &[0x7E, 0x7E]);
//! assert_eq!(*bytes.last().unwrap(), 0x0A);
//!
//! // Parse it back
//! let parsed = ControlFrame::decode(&bytes).unwrap();
//! assert_eq!(parsed.cmd, OutgoingCommand::Ctrl2A);
//! ```

#![deny(missing_docs)]

pub mod commands;
pub mod control;
pub mod crc;
pub mod data;
pub mod error;
pub mod transport;

pub use error::{Error, Result};

/// Protocol constants shared by both channels.
pub mod consts {
    /// UDP port the radar data/control usually listens on (per radar `nwk.port`).
    pub const DEFAULT_TCP_PORT: u16 = 5001;
    /// Default UDP port used by the app (`system.nwk.udpPort`).
    pub const DEFAULT_UDP_PORT: u16 = 5002;

    /// Control frame start-of-frame byte.
    pub const SOF: u8 = 0x7E;
    /// Control frame terminator bytes.
    pub const EOF: [u8; 2] = [0x0D, 0x0A];

    /// Minimum valid data frame length.
    pub const DATA_MIN_LEN: usize = 0x15; // 21
    /// Data frame header size.
    pub const DATA_HEADER_LEN: usize = 16;
    /// Size of the CRC trailer.
    pub const DATA_CRC_LEN: usize = 2;
}
