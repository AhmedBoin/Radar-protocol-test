//! CRC-16/MODBUS — used by the radar **data** channel.
//!
//! * Polynomial: `0xA001` (bit-reflected `0x8005`)
//! * Init: `0xFFFF`
//! * Reflected in/out, **no** final XOR
//!
//! The CRC is computed over `frame[0 .. len-2]` and stored in the last 2 bytes.

/// Compute CRC-16/MODBUS over `data`.
///
/// ```
/// use fdad_radar::crc::crc16_modbus;
/// // Classic check value for the ASCII string "123456789" is 0x4B37
/// assert_eq!(crc16_modbus(b"123456789"), 0x4B37);
/// ```
pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_value() {
        assert_eq!(crc16_modbus(b"123456789"), 0x4B37);
    }

    #[test]
    fn empty() {
        assert_eq!(crc16_modbus(&[]), 0xFFFF);
    }
}
