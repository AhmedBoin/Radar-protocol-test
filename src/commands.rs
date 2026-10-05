//! Command codes and the control-channel payload structs.
//!
//! Outgoing frames carry an *internal* code that is remapped to a *wire* byte:
//! internal `0x1A` -> `0x21`, internal `0x2A` -> `0x23`.
//! Incoming frames are dispatched on `0xA1` / `0xA2`.

use crate::error::{Error, Result};

/// Internal command code for wire `0x21`.
pub const CMD_INTERNAL_1A: u8 = 0x1A;
/// Internal command code for wire `0x23`.
pub const CMD_INTERNAL_2A: u8 = 0x2A;

/// Wire command byte produced by internal `0x1A`.
pub const CMD_WIRE_21: u8 = 0x21;
/// Wire command byte produced by internal `0x2A`.
pub const CMD_WIRE_23: u8 = 0x23;

/// Incoming command byte A1.
pub const CMD_RX_A1: u8 = 0xA1;
/// Incoming command byte A2.
pub const CMD_RX_A2: u8 = 0xA2;

/// Outgoing command types (app -> radar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutgoingCommand {
    /// Internal `0x1A` / wire `0x21`.
    Ctrl1A,
    /// Internal `0x2A` / wire `0x23`.
    Ctrl2A,
}

impl OutgoingCommand {
    /// The internal code before remapping.
    pub fn internal_code(self) -> u8 {
        match self {
            OutgoingCommand::Ctrl1A => CMD_INTERNAL_1A,
            OutgoingCommand::Ctrl2A => CMD_INTERNAL_2A,
        }
    }

    /// The byte that actually goes on the wire.
    pub fn wire_code(self) -> u8 {
        match self {
            OutgoingCommand::Ctrl1A => CMD_WIRE_21,
            OutgoingCommand::Ctrl2A => CMD_WIRE_23,
        }
    }

    /// Map an internal code to the outgoing command.
    pub fn from_internal(code: u8) -> Result<Self> {
        match code {
            CMD_INTERNAL_1A => Ok(OutgoingCommand::Ctrl1A),
            CMD_INTERNAL_2A => Ok(OutgoingCommand::Ctrl2A),
            other => Err(Error::UnknownCommand(other)),
        }
    }

    /// Map a wire code (`0x21` / `0x23`) back to the outgoing command.
    pub fn from_wire(code: u8) -> Result<Self> {
        match code {
            CMD_WIRE_21 => Ok(OutgoingCommand::Ctrl1A),
            CMD_WIRE_23 => Ok(OutgoingCommand::Ctrl2A),
            other => Err(Error::UnknownCommand(other)),
        }
    }
}

/// Incoming command types (radar -> app).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomingCommand {
    /// `0xA1` — small status/config block, see `StatusA1`.
    StatusA1,
    /// `0xA2` — small status block.
    StatusA2,
}

impl IncomingCommand {
    /// Map a wire command byte to an incoming command.
    pub fn from_wire(code: u8) -> Result<Self> {
        match code {
            CMD_RX_A1 => Ok(IncomingCommand::StatusA1),
            CMD_RX_A2 => Ok(IncomingCommand::StatusA2),
            other => Err(Error::UnknownCommand(other)),
        }
    }

    /// The wire byte.
    pub fn wire_code(self) -> u8 {
        match self {
            IncomingCommand::StatusA1 => CMD_RX_A1,
            IncomingCommand::StatusA2 => CMD_RX_A2,
        }
    }
}

/// Payload of incoming command `0xA1` (big-endian, in order).
///
/// Field *types* are confirmed from the binary; the semantic names are
/// best-effort and should be validated against a live capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusA1 {
    /// i8  @ obj+0x98
    pub f0: i8,
    /// i8  @ obj+0x99
    pub f1: i8,
    /// i16 @ obj+0x9A
    pub f2: i16,
    /// i16 @ obj+0x9C
    pub f3: i16,
    /// i16 @ obj+0x9E
    pub f4: i16,
    /// i16 @ obj+0xA0
    pub f5: i16,
    /// i16 @ obj+0xA2
    pub f6: i16,
    /// i8  @ obj+0xA4
    pub f7: i8,
    /// i16 @ obj+0xA6
    pub f8: i16,
}

impl StatusA1 {
    /// Exact payload size in bytes.
    pub const LEN: usize = 15;

    /// Decode from a big-endian payload.
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() < Self::LEN {
            return Err(Error::TruncatedPayload { need: Self::LEN, have: b.len() });
        }
        Ok(StatusA1 {
            f0: b[0] as i8,
            f1: b[1] as i8,
            f2: i16::from_be_bytes([b[2], b[3]]),
            f3: i16::from_be_bytes([b[4], b[5]]),
            f4: i16::from_be_bytes([b[6], b[7]]),
            f5: i16::from_be_bytes([b[8], b[9]]),
            f6: i16::from_be_bytes([b[10], b[11]]),
            f7: b[12] as i8,
            f8: i16::from_be_bytes([b[13], b[14]]),
        })
    }

    /// Encode to a big-endian payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(Self::LEN);
        v.push(self.f0 as u8);
        v.push(self.f1 as u8);
        v.extend_from_slice(&self.f2.to_be_bytes());
        v.extend_from_slice(&self.f3.to_be_bytes());
        v.extend_from_slice(&self.f4.to_be_bytes());
        v.extend_from_slice(&self.f5.to_be_bytes());
        v.extend_from_slice(&self.f6.to_be_bytes());
        v.push(self.f7 as u8);
        v.extend_from_slice(&self.f8.to_be_bytes());
        v
    }
}

/// Radar hardware / scan parameters (per radar `hardware{}` in `setting.json`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RadarHardware {
    /// Operating frequency / channel.
    pub freq: i32,
    /// CFAR threshold.
    pub cfar: i32,
    /// Rotation / sweep speed setting.
    pub speed: i32,
    /// RCS setting.
    pub rcs: i32,
    /// Working mode (0 = circumference sweep, 1 = fan sweep).
    pub working_mode: i32,
    /// Scan range low (m).
    pub scan_range_low: f64,
    /// Scan range high (m).
    pub scan_range_high: f64,
    /// Azimuth low (deg).
    pub angle_range_low: f64,
    /// Azimuth high (deg).
    pub angle_range_high: f64,
    /// Height low (m).
    pub height_range_low: f64,
    /// Height high (m).
    pub height_range_high: f64,
    /// Speed low (m/s).
    pub speed_range_low: f64,
    /// Speed high (m/s).
    pub speed_range_high: f64,
    /// RCS low (m^2).
    pub rcs_range_low: f64,
    /// RCS high (m^2).
    pub rcs_range_high: f64,
}

/// Radar working mode (UI: "Cir sweep Ns" / "Fan sweep" / "Stand by").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingMode {
    /// 360-degree circumference sweep.
    CircumferenceSweep,
    /// Sector (fan) sweep.
    FanSweep,
    /// Not rotating.
    StandBy,
}

impl WorkingMode {
    /// Map the numeric `working_mode` field.
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(WorkingMode::CircumferenceSweep),
            1 => Some(WorkingMode::FanSweep),
            2 => Some(WorkingMode::StandBy),
            _ => None,
        }
    }
}

