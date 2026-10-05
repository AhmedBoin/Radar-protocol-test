//! Crate error type.

use std::fmt;

/// Errors returned by the protocol layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A frame was shorter than the minimum required length.
    FrameTooShort {
        /// Actual byte length seen.
        len: usize,
        /// Minimum required.
        min: usize,
    },
    /// Control frame did not start with `7E 7E`.
    BadSof,
    /// Control frame did not end with `0D 0A`.
    BadEof,
    /// Control-frame checksum mismatch.
    BadChecksum {
        /// Checksum computed locally.
        expected: u8,
        /// Checksum found in the frame.
        found: u8,
    },
    /// Data-frame CRC-16/MODBUS mismatch.
    BadCrc {
        /// CRC computed locally.
        expected: u16,
        /// CRC found in the frame.
        found: u16,
    },
    /// The frame carried a command code this crate does not know.
    UnknownCommand(u8),
    /// The data frame carried an unknown type code.
    UnknownFrameType(u16),
    /// The payload was shorter than the struct it should contain.
    TruncatedPayload {
        /// Bytes required.
        need: usize,
        /// Bytes available.
        have: usize,
    },
    /// Underlying I/O error.
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FrameTooShort { len, min } => {
                write!(f, "frame too short: {len} bytes (min {min})")
            }
            Error::BadSof => write!(f, "control frame missing SOF (7E 7E)"),
            Error::BadEof => write!(f, "control frame missing EOF (0D 0A)"),
            Error::BadChecksum { expected, found } => write!(
                f,
                "control checksum mismatch: computed 0x{expected:02X}, frame has 0x{found:02X}"
            ),
            Error::BadCrc { expected, found } => write!(
                f,
                "CRC-16 mismatch: computed 0x{expected:04X}, frame has 0x{found:04X}"
            ),
            Error::UnknownCommand(c) => write!(f, "unknown command 0x{c:02X}"),
            Error::UnknownFrameType(t) => write!(f, "unknown data frame type 0x{t:02X}"),
            Error::TruncatedPayload { need, have } => {
                write!(f, "truncated payload: need {need} bytes, have {have}")
            }
            Error::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;
