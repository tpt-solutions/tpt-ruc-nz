// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! UBX frames that set the GNSS receiver's output rate.
//!
//! The u-blox NEO-M8N takes its configuration as UBX binary frames on the same
//! UART that carries NMEA. This module builds the UBX-CFG-RATE frame for a
//! given output rate. It only builds bytes. The firmware writes them to the
//! UART, and nothing here touches hardware.
//!
//! The rate is set in RAM. It is lost on power-off unless the receiver's
//! configuration is saved with UBX-CFG-CFG, which this module does not send.

use crate::control::{Command, GNSS_RATES_HZ};

/// UBX sync bytes that start every frame.
pub const SYNC: [u8; 2] = [0xB5, 0x62];
/// UBX class for configuration messages.
pub const CLASS_CFG: u8 = 0x06;
/// UBX-CFG-RATE message ID within [`CLASS_CFG`].
pub const ID_CFG_RATE: u8 = 0x08;
/// Frame length for UBX-CFG-RATE: sync (2), class and ID (2), length (2), payload (6), checksum (2).
pub const CFG_RATE_FRAME_LEN: usize = 14;

/// Builds the UBX-CFG-RATE frame that sets the navigation solution to `hz` Hz.
///
/// Returns `None` for a rate outside [`GNSS_RATES_HZ`], so an unsupported rate
/// never reaches the receiver.
pub fn cfg_rate_frame(hz: u8) -> Option<[u8; CFG_RATE_FRAME_LEN]> {
    if !GNSS_RATES_HZ.contains(&hz) {
        return None;
    }
    // Measurement period in milliseconds: 1 Hz is 1000 ms, 5 Hz is 200 ms.
    let meas_rate_ms = 1000u16 / u16::from(hz);
    let [meas_lo, meas_hi] = meas_rate_ms.to_le_bytes();
    // Navigation rate 1 means one solution per measurement. timeRef 1 is GPS time.
    let payload = [meas_lo, meas_hi, 0x01, 0x00, 0x01, 0x00];
    Some(frame(CLASS_CFG, ID_CFG_RATE, &payload))
}

/// The UBX frame the UART must send for a control command, or `None` when the
/// command does not change the receiver.
///
/// The firmware writes the frame to the GNSS UART after `Settings::apply` accepts
/// the command. Commands that only change the trip filter return `None`.
pub fn frame_for_command(command: Command) -> Option<[u8; CFG_RATE_FRAME_LEN]> {
    match command {
        Command::SetGnssRate { hz } => cfg_rate_frame(hz),
        Command::RequestSegments
        | Command::AcknowledgeSegments { .. }
        | Command::SetMaxHdop { .. }
        | Command::SetMinMove { .. } => None,
    }
}

/// Wraps a payload in a UBX frame with its Fletcher checksum.
fn frame(class: u8, id: u8, payload: &[u8; 6]) -> [u8; CFG_RATE_FRAME_LEN] {
    let len = payload.len() as u16;
    let [len_lo, len_hi] = len.to_le_bytes();
    let mut out = [0u8; CFG_RATE_FRAME_LEN];
    out[..2].copy_from_slice(&SYNC);
    out[2] = class;
    out[3] = id;
    out[4] = len_lo;
    out[5] = len_hi;
    out[6..12].copy_from_slice(payload);
    // The checksum covers class through the end of the payload.
    let (ck_a, ck_b) = fletcher8(&out[2..12]);
    out[12] = ck_a;
    out[13] = ck_b;
    out
}

/// The two-byte Fletcher-8 checksum that UBX uses.
pub fn fletcher8(bytes: &[u8]) -> (u8, u8) {
    let mut ck_a: u8 = 0;
    let mut ck_b: u8 = 0;
    for &byte in bytes {
        ck_a = ck_a.wrapping_add(byte);
        ck_b = ck_b.wrapping_add(ck_a);
    }
    (ck_a, ck_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_hertz_frame_matches_the_documented_bytes() {
        // Known-good UBX-CFG-RATE frame for 1 Hz (measRate 1000 ms).
        assert_eq!(
            cfg_rate_frame(1),
            Some([
                0xB5, 0x62, 0x06, 0x08, 0x06, 0x00, 0xE8, 0x03, 0x01, 0x00, 0x01, 0x00, 0x01, 0x39,
            ])
        );
    }

    #[test]
    fn five_hertz_frame_uses_a_200_ms_period() {
        assert_eq!(
            cfg_rate_frame(5),
            Some([
                0xB5, 0x62, 0x06, 0x08, 0x06, 0x00, 0xC8, 0x00, 0x01, 0x00, 0x01, 0x00, 0xDE, 0x6A,
            ])
        );
    }

    #[test]
    fn rates_outside_the_allowed_list_build_no_frame() {
        assert_eq!(cfg_rate_frame(0), None);
        assert_eq!(cfg_rate_frame(2), None);
        assert_eq!(cfg_rate_frame(10), None);
    }

    #[test]
    fn every_allowed_rate_has_a_consistent_checksum() {
        for hz in GNSS_RATES_HZ {
            let frame = cfg_rate_frame(hz).expect("allowed rate builds a frame");
            let (ck_a, ck_b) = fletcher8(&frame[2..12]);
            assert_eq!((frame[12], frame[13]), (ck_a, ck_b));
        }
    }

    #[test]
    fn only_the_rate_command_changes_the_receiver() {
        assert_eq!(
            frame_for_command(Command::SetGnssRate { hz: 5 }),
            cfg_rate_frame(5)
        );
        assert_eq!(frame_for_command(Command::SetMaxHdop { tenths: 25 }), None);
        assert_eq!(frame_for_command(Command::SetMinMove { metres: 5 }), None);
        assert_eq!(frame_for_command(Command::RequestSegments), None);
        assert_eq!(
            frame_for_command(Command::AcknowledgeSegments { count: 1 }),
            None
        );
    }

    #[test]
    fn fletcher8_of_the_empty_input_is_zero() {
        assert_eq!(fletcher8(&[]), (0, 0));
    }
}
