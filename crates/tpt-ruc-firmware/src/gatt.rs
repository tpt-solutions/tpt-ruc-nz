// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Handling of one write to the control characteristic.
//!
//! The BLE stack hands over the written bytes. This module checks them with
//! [`control::parse`], applies accepted settings, and returns the status byte
//! to notify, plus the action the firmware must take. It holds no radio or
//! timer state, so it works with any stack and runs on the host.
//!
//! A refused write changes nothing and returns an error status. The status
//! `0x80` means the device parsed and accepted the command. It does not mean
//! the work is done. An acknowledgement the transfer session refuses is
//! reported by a later `0x8F` notification (see `docs/ble-design.md`).

use crate::control::{self, Command, Settings, RESP_OK};

/// What the firmware must do after an accepted write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing beyond the status byte.
    None,
    /// Start a segment transfer from the oldest stored segment.
    RequestSegments,
    /// The phone holds this many segments of the current transfer.
    AcknowledgeSegments(u32),
    /// Settings changed. Apply them to the GNSS receiver and to the recorder.
    SettingsChanged(Settings),
}

/// The result of one control write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reply {
    /// Status byte to notify on the control characteristic.
    pub status: u8,
    /// What the firmware must do.
    pub action: Action,
}

/// Handles one write to the control characteristic, updating `settings` on success.
pub fn handle_write(settings: &mut Settings, bytes: &[u8]) -> Reply {
    let refused = |status| Reply {
        status,
        action: Action::None,
    };
    let command = match control::parse(bytes) {
        Ok(command) => command,
        Err(error) => return refused(error.response()),
    };
    // `apply` checks the range before it stores anything, so a refused value leaves `settings` alone.
    if let Err(error) = settings.apply(command) {
        return refused(error.response());
    }
    let action = match command {
        Command::RequestSegments => Action::RequestSegments,
        Command::AcknowledgeSegments { count } => Action::AcknowledgeSegments(count),
        Command::SetGnssRate { .. } | Command::SetMaxHdop { .. } | Command::SetMinMove { .. } => {
            Action::SettingsChanged(*settings)
        }
    };
    Reply {
        status: RESP_OK,
        action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{
        CMD_ACK_SEGMENTS, CMD_REQUEST_SEGMENTS, CMD_SET_GNSS_RATE, CMD_SET_MAX_HDOP, RESP_INVALID,
    };

    #[test]
    fn request_and_ack_return_ok_with_their_action() {
        let mut settings = Settings::default();
        assert_eq!(
            handle_write(&mut settings, &[CMD_REQUEST_SEGMENTS]),
            Reply {
                status: RESP_OK,
                action: Action::RequestSegments
            }
        );
        assert_eq!(
            handle_write(&mut settings, &[CMD_ACK_SEGMENTS, 42, 0, 0, 0]),
            Reply {
                status: RESP_OK,
                action: Action::AcknowledgeSegments(42)
            }
        );
    }

    #[test]
    fn an_accepted_setting_changes_the_settings_and_reports_them() {
        let mut settings = Settings::default();
        let reply = handle_write(&mut settings, &[CMD_SET_MAX_HDOP, 25]);
        assert_eq!(reply.status, RESP_OK);
        assert_eq!(settings.max_hdop_tenths, 25);
        assert_eq!(reply.action, Action::SettingsChanged(settings));
    }

    #[test]
    fn a_refused_setting_changes_nothing_and_reports_invalid() {
        let mut settings = Settings::default();
        let before = settings;
        let reply = handle_write(&mut settings, &[CMD_SET_GNSS_RATE, 10]);
        assert_eq!(
            reply,
            Reply {
                status: RESP_INVALID,
                action: Action::None
            }
        );
        assert_eq!(settings, before);
    }

    #[test]
    fn malformed_writes_are_refused_without_side_effects() {
        let mut settings = Settings::default();
        let before = settings;
        let cases: [&[u8]; 4] = [
            &[],
            &[0x77],
            &[CMD_ACK_SEGMENTS, 1],
            &[CMD_REQUEST_SEGMENTS, 0],
        ];
        for bytes in cases {
            let reply = handle_write(&mut settings, bytes);
            assert_eq!(reply.status, RESP_INVALID, "{bytes:?}");
            assert_eq!(reply.action, Action::None, "{bytes:?}");
        }
        assert_eq!(settings, before);
    }
}
