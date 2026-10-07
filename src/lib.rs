//! # fdad-radar — FDAD‑DCT radar **TCP** protocol
//!
//! Reverse‑engineered from `FDAD-DCTv3.0.0.exe` and verified against a live BWR‑T15.
//!
//! ## Transport
//! Plain **TCP** (no TLS); the app is the client, radar at `ip:5001`.
//!
//! ## Frame (both directions, multi‑byte ints big‑endian)
//! ```text
//! 55 AA 55 AA | LEN (u32 BE = payload+2) | PAYLOAD (LEN-2) | CRC16-MODBUS(payload) (u16 LE)
//! total bytes = LEN + 8
//! ```
//!
//! ## Messages — payload starts with `type(u32) | counter(u32)`
//!
//! Enable the optional **`serde`** feature to derive `Serialize`/`Deserialize`
//! (camelCase JSON) on all types, for Tauri v2 / JS frontends. See `TAURI.md`.
//! | type | dir | meaning |
//! |---|---|---|
//! | `0x00000001` | app→radar | heartbeat |
//! | `0x00000040` | app→radar | set registers |
//! | `0x00000080` | app→radar | set JSON settings |
//! | `0x00000082` | app→radar | get JSON |
//! | `0x00030002` | radar→app | data stream (sweep + tracks) |
//!
//! ## Example
//! ```
//! use fdad_radar::messages::heartbeat;
//! use fdad_radar::frame::Frame;
//! let wire = Frame::new(heartbeat(1)).encode();
//! assert_eq!(&wire[0..4], &[0x55, 0xAA, 0x55, 0xAA]);
//! ```

#![deny(missing_docs)]

pub mod client;
pub mod crc;
pub mod error;
pub mod frame;
pub mod messages;

pub use client::RadarClient;
pub use error::{Error, Result};
pub use frame::Frame;
pub use messages::{
    data_message, parse_data, parse_data_message, parse_records, DataHeader, DataMessage, Message,
    MessageType, Record, Sweep, Target, RECORD_LEN,
};

/// Protocol constants.
pub mod consts {
    /// Frame magic `55 AA 55 AA`.
    pub const MAGIC: u32 = crate::frame::MAGIC;
    /// Default radar TCP port.
    pub const DEFAULT_TCP_PORT: u16 = 5001;
}
