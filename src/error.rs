//! Crate error type.

use std::fmt;

/// Errors returned by the protocol layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A frame was shorter than required.
    FrameTooShort {
        /// Bytes available.
        len: usize,
        /// Bytes required.
        min: usize,
    },
    /// Frame did not start with `55 AA 55 AA`.
    BadMagic(u32),
    /// CRC-16/MODBUS mismatch.
    BadCrc {
        /// CRC computed locally.
        expected: u16,
        /// CRC found in the frame.
        found: u16,
    },
    /// Payload shorter than the required struct.
    TruncatedPayload {
        /// Bytes required.
        need: usize,
        /// Bytes available.
        have: usize,
    },
    /// The `SetJson` body was not valid UTF-8/ASCII.
    BadJson,
    /// Underlying I/O error.
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FrameTooShort { len, min } => {
                write!(f, "frame too short: {len} bytes (need {min})")
            }
            Error::BadMagic(m) => write!(f, "bad frame magic 0x{m:08X} (want 0x55AA55AA)"),
            Error::BadCrc { expected, found } => {
                write!(
                    f,
                    "CRC-16 mismatch: computed 0x{expected:04X}, frame has 0x{found:04X}"
                )
            }
            Error::TruncatedPayload { need, have } => {
                write!(f, "truncated payload: need {need} bytes, have {have}")
            }
            Error::BadJson => write!(f, "invalid JSON body"),
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
