// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Version 1 BLE payload for one GNSS fix, encoded as MessagePack.
//!
//! The payload is a fixed-order array:
//!
//! `[version, time_ms, lat_e7, lon_e7, speed_cm_s, fix_quality]`
//!
//! - `version`: always `1` for this layout.
//! - `time_ms`: Unix milliseconds, UTC.
//! - `lat_e7`, `lon_e7`: degrees times 10^7, signed.
//! - `speed_cm_s`: speed over ground in cm/s, or nil when the receiver gave none.
//! - `fix_quality`: 0 no fix, 1 GPS, 2 DGPS, 3 PPS, 4 RTK fixed, 5 RTK float,
//!   6 estimated, 7 manual, 8 simulation.
//!
//! Any change to this layout needs a new version number.

use rmp::encode;
use tpt_ruc_core::{Fix, FixQuality};

/// Version written into every payload produced by this module.
pub const PAYLOAD_VERSION: u8 = 1;

/// Number of array elements in a version 1 payload.
pub const ARRAY_LEN: u32 = 6;

/// Why a payload could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The output buffer is too small for the payload.
    BufferTooSmall,
}

/// The fix quality code used in version 1 payloads.
pub const fn quality_code(quality: FixQuality) -> u8 {
    match quality {
        FixQuality::NoFix => 0,
        FixQuality::Gps => 1,
        FixQuality::Dgps => 2,
        FixQuality::Pps => 3,
        FixQuality::RtkFixed => 4,
        FixQuality::RtkFloat => 5,
        FixQuality::Estimated => 6,
        FixQuality::Manual => 7,
        FixQuality::Simulation => 8,
    }
}

/// Encodes a fix as a version 1 payload into `buf`. Returns the bytes written.
pub fn encode_fix_v1(fix: &Fix, buf: &mut [u8]) -> Result<usize, EncodeError> {
    let mut cursor: &mut [u8] = buf;
    let total = cursor.len();
    write_fix(&mut cursor, fix)?;
    Ok(total - cursor.len())
}

fn write_fix(out: &mut &mut [u8], fix: &Fix) -> Result<(), EncodeError> {
    // A slice writer can only fail by running out of space, so every error maps to one variant.
    let full = |_| EncodeError::BufferTooSmall;
    encode::write_array_len(out, ARRAY_LEN).map_err(full)?;
    encode::write_uint(out, u64::from(PAYLOAD_VERSION)).map_err(full)?;
    encode::write_uint(out, fix.time_ms).map_err(full)?;
    encode::write_sint(out, degrees_e7(fix.coord.lat.degrees())).map_err(full)?;
    encode::write_sint(out, degrees_e7(fix.coord.lon.degrees())).map_err(full)?;
    match fix.speed_mps {
        Some(speed) => {
            encode::write_uint(out, speed_cm_s(speed)).map_err(full)?;
        }
        None => {
            encode::write_nil(out).map_err(|_| EncodeError::BufferTooSmall)?;
        }
    }
    encode::write_uint(out, u64::from(quality_code(fix.fix))).map_err(full)?;
    Ok(())
}

fn degrees_e7(degrees: f64) -> i64 {
    libm::round(degrees * 1e7) as i64
}

fn speed_cm_s(mps: f64) -> u64 {
    // Negative speeds cannot come from a receiver, but clamp rather than wrap.
    libm::round(mps.max(0.0) * 100.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp::decode;
    use tpt_ruc_core::Coordinate;

    fn fix(speed: Option<f64>) -> Fix {
        Fix {
            coord: Coordinate::new(-41.2865, 174.7762).unwrap(),
            time_ms: 1_700_000_000_123,
            fix: FixQuality::Gps,
            hdop: Some(0.9),
            speed_mps: speed,
        }
    }

    /// Reads back the six fields of a version 1 payload.
    fn decode_fields(bytes: &[u8]) -> (u64, u64, i64, i64, Option<u64>, u64) {
        let mut reader = bytes;
        let len = decode::read_array_len(&mut reader).unwrap();
        assert_eq!(len, ARRAY_LEN);
        let version: u64 = decode::read_int(&mut reader).unwrap();
        let time: u64 = decode::read_int(&mut reader).unwrap();
        let lat: i64 = decode::read_int(&mut reader).unwrap();
        let lon: i64 = decode::read_int(&mut reader).unwrap();
        let speed = if reader[0] == 0xc0 {
            decode::read_nil(&mut reader).unwrap();
            None
        } else {
            Some(decode::read_int(&mut reader).unwrap())
        };
        let quality: u64 = decode::read_int(&mut reader).unwrap();
        (version, time, lat, lon, speed, quality)
    }

    #[test]
    fn encodes_the_version_1_fields() {
        let mut buf = [0u8; 64];
        let n = encode_fix_v1(&fix(Some(12.5)), &mut buf).unwrap();
        let (version, time, lat, lon, speed, quality) = decode_fields(&buf[..n]);
        assert_eq!(version, 1);
        assert_eq!(time, 1_700_000_000_123);
        assert_eq!(lat, -412_865_000);
        assert_eq!(lon, 1_747_762_000);
        assert_eq!(speed, Some(1250));
        assert_eq!(quality, 1);
    }

    #[test]
    fn missing_speed_is_nil() {
        let mut buf = [0u8; 64];
        let n = encode_fix_v1(&fix(None), &mut buf).unwrap();
        let (_, _, _, _, speed, _) = decode_fields(&buf[..n]);
        assert_eq!(speed, None);
    }

    #[test]
    fn small_buffer_is_reported_not_panicked() {
        let mut buf = [0u8; 4];
        assert_eq!(
            encode_fix_v1(&fix(Some(1.0)), &mut buf),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn quality_codes_follow_the_documented_table() {
        assert_eq!(quality_code(FixQuality::NoFix), 0);
        assert_eq!(quality_code(FixQuality::Gps), 1);
        assert_eq!(quality_code(FixQuality::RtkFixed), 4);
        assert_eq!(quality_code(FixQuality::Simulation), 8);
    }

    #[test]
    fn golden_bytes_for_version_1() {
        // Pins the wire bytes, so a layout change shows up as a test failure.
        let mut buf = [0u8; 64];
        let n = encode_fix_v1(&fix(None), &mut buf).unwrap();
        assert_eq!(buf[0], 0x96, "fixarray of six elements");
        assert_eq!(buf[1], 0x01, "version 1");
        assert_eq!(buf[n - 2], 0xc0, "speed is nil");
        assert_eq!(buf[n - 1], 0x01, "fix quality GPS");
    }
}
