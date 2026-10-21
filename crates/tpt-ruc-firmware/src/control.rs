// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Control commands written by the phone, and the device settings they change.
//!
//! The command bytes follow `docs/ble-design.md`. Set GNSS rate (`0x10`) is
//! the only setting in that table. Two more settings are added here for the
//! trip filter, and `docs/ble-design.md` must list them before a release:
//!
//! | Command | Byte | Argument | Bounds |
//! |---|---|---|---|
//! | Set max HDOP | `0x11` | `u8`, tenths of HDOP | 10 to 100 (1.0 to 10.0) |
//! | Set minimum move | `0x12` | `u8`, metres | 1 to 50 |
//!
//! The parser and the validation are pure. Nothing here touches hardware. The
//! firmware applies the result to the GNSS receiver and the trip tracker.

use tpt_ruc_core::trip::Config as TrackerConfig;

/// Request the segment transfer.
pub const CMD_REQUEST_SEGMENTS: u8 = 0x01;
/// Acknowledge segments up to a count.
pub const CMD_ACK_SEGMENTS: u8 = 0x02;
/// Set the GNSS output rate.
pub const CMD_SET_GNSS_RATE: u8 = 0x10;
/// Set the largest HDOP accepted.
pub const CMD_SET_MAX_HDOP: u8 = 0x11;
/// Set the distance a stationary receiver must move before drift stops being ignored.
pub const CMD_SET_MIN_MOVE: u8 = 0x12;

/// Success response.
pub const RESP_OK: u8 = 0x80;
/// Unknown or invalid command response.
pub const RESP_INVALID: u8 = 0x8F;

/// Allowed GNSS output rates in Hz.
pub const GNSS_RATES_HZ: [u8; 2] = [1, 5];
/// Lowest and highest HDOP accepted, in tenths.
pub const HDOP_TENTHS_RANGE: (u8, u8) = (10, 100);
/// Smallest and largest minimum move, in metres.
pub const MIN_MOVE_M_RANGE: (u8, u8) = (1, 50);

/// A parsed command, with its arguments checked for type but not yet for range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Start a segment transfer.
    RequestSegments,
    /// Drop the first `count` segments from the queue.
    AcknowledgeSegments {
        /// Number of segments the phone has stored.
        count: u32,
    },
    /// Set the GNSS output rate in Hz.
    SetGnssRate {
        /// Requested rate.
        hz: u8,
    },
    /// Set the largest HDOP accepted, in tenths.
    SetMaxHdop {
        /// Requested HDOP times ten.
        tenths: u8,
    },
    /// Set the minimum move, in metres.
    SetMinMove {
        /// Requested distance.
        metres: u8,
    },
}

/// Why a command was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    /// The command byte is not one this device knows.
    UnknownCommand(u8),
    /// The command needs more argument bytes than were sent.
    MissingArgument,
    /// An argument is outside its allowed bounds.
    OutOfRange,
}

impl CommandError {
    /// The response byte the device sends back for this error.
    pub fn response(self) -> u8 {
        RESP_INVALID
    }
}

/// Reads one command from the bytes written to the control characteristic.
///
/// Extra bytes after a complete command are refused, so a truncated or padded
/// write cannot be mistaken for a valid command.
pub fn parse(bytes: &[u8]) -> Result<Command, CommandError> {
    let (&command, args) = bytes.split_first().ok_or(CommandError::MissingArgument)?;
    let (command, expected) = match command {
        CMD_REQUEST_SEGMENTS => (Command::RequestSegments, 0),
        CMD_ACK_SEGMENTS => {
            let count = u32::from_le_bytes(take::<4>(args)?);
            (Command::AcknowledgeSegments { count }, 4)
        }
        CMD_SET_GNSS_RATE => {
            let [hz] = take::<1>(args)?;
            (Command::SetGnssRate { hz }, 1)
        }
        CMD_SET_MAX_HDOP => {
            let [tenths] = take::<1>(args)?;
            (Command::SetMaxHdop { tenths }, 1)
        }
        CMD_SET_MIN_MOVE => {
            let [metres] = take::<1>(args)?;
            (Command::SetMinMove { metres }, 1)
        }
        other => return Err(CommandError::UnknownCommand(other)),
    };
    if args.len() != expected {
        return Err(if args.len() < expected {
            CommandError::MissingArgument
        } else {
            CommandError::OutOfRange
        });
    }
    Ok(command)
}

/// The first `N` bytes of `args`, or `MissingArgument` when there are too few.
fn take<const N: usize>(args: &[u8]) -> Result<[u8; N], CommandError> {
    args.get(..N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(CommandError::MissingArgument)
}

/// Device settings the phone may change over BLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// GNSS output rate in Hz, one of [`GNSS_RATES_HZ`].
    pub gnss_hz: u8,
    /// Largest HDOP accepted, in tenths, within [`HDOP_TENTHS_RANGE`].
    pub max_hdop_tenths: u8,
    /// Minimum move in metres, within [`MIN_MOVE_M_RANGE`].
    pub min_move_m: u8,
}

impl Default for Settings {
    /// Matches the core tracker defaults: 1 Hz, HDOP 5.0, 5 m.
    fn default() -> Self {
        Self {
            gnss_hz: 1,
            max_hdop_tenths: 50,
            min_move_m: 5,
        }
    }
}

impl Settings {
    /// Applies a settings command. Commands that do not change settings are accepted and ignored.
    pub fn apply(&mut self, command: Command) -> Result<(), CommandError> {
        match command {
            Command::SetGnssRate { hz } => {
                if !GNSS_RATES_HZ.contains(&hz) {
                    return Err(CommandError::OutOfRange);
                }
                self.gnss_hz = hz;
            }
            Command::SetMaxHdop { tenths } => {
                if !(HDOP_TENTHS_RANGE.0..=HDOP_TENTHS_RANGE.1).contains(&tenths) {
                    return Err(CommandError::OutOfRange);
                }
                self.max_hdop_tenths = tenths;
            }
            Command::SetMinMove { metres } => {
                if !(MIN_MOVE_M_RANGE.0..=MIN_MOVE_M_RANGE.1).contains(&metres) {
                    return Err(CommandError::OutOfRange);
                }
                self.min_move_m = metres;
            }
            Command::RequestSegments | Command::AcknowledgeSegments { .. } => {}
        }
        Ok(())
    }

    /// The trip tracker settings these device settings imply. Everything else keeps the core default.
    pub fn tracker_config(&self) -> TrackerConfig {
        TrackerConfig {
            max_hdop: f64::from(self.max_hdop_tenths) / 10.0,
            min_move_m: f64::from(self.min_move_m),
            ..TrackerConfig::default()
        }
    }
}

/// The response byte for the outcome of a command.
pub fn response_for(outcome: Result<(), CommandError>) -> u8 {
    match outcome {
        Ok(()) => RESP_OK,
        Err(error) => error.response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_documented_command() {
        assert_eq!(parse(&[CMD_REQUEST_SEGMENTS]), Ok(Command::RequestSegments));
        assert_eq!(
            parse(&[CMD_ACK_SEGMENTS, 0x2a, 0x00, 0x00, 0x00]),
            Ok(Command::AcknowledgeSegments { count: 42 })
        );
        assert_eq!(
            parse(&[CMD_SET_GNSS_RATE, 5]),
            Ok(Command::SetGnssRate { hz: 5 })
        );
        assert_eq!(
            parse(&[CMD_SET_MAX_HDOP, 25]),
            Ok(Command::SetMaxHdop { tenths: 25 })
        );
        assert_eq!(
            parse(&[CMD_SET_MIN_MOVE, 3]),
            Ok(Command::SetMinMove { metres: 3 })
        );
    }

    #[test]
    fn unknown_commands_are_refused_with_their_byte() {
        assert_eq!(parse(&[0x77]), Err(CommandError::UnknownCommand(0x77)));
        assert_eq!(parse(&[0x77]).unwrap_err().response(), RESP_INVALID);
    }

    #[test]
    fn short_arguments_are_missing_and_long_ones_are_out_of_range() {
        assert_eq!(parse(&[]), Err(CommandError::MissingArgument));
        assert_eq!(
            parse(&[CMD_ACK_SEGMENTS, 1]),
            Err(CommandError::MissingArgument)
        );
        assert_eq!(
            parse(&[CMD_SET_GNSS_RATE]),
            Err(CommandError::MissingArgument)
        );
        assert_eq!(
            parse(&[CMD_SET_GNSS_RATE, 1, 0]),
            Err(CommandError::OutOfRange)
        );
        assert_eq!(
            parse(&[CMD_REQUEST_SEGMENTS, 0]),
            Err(CommandError::OutOfRange)
        );
    }

    #[test]
    fn gnss_rate_accepts_only_the_listed_rates() {
        let mut settings = Settings::default();
        assert_eq!(settings.apply(Command::SetGnssRate { hz: 5 }), Ok(()));
        assert_eq!(settings.gnss_hz, 5);
        assert_eq!(
            settings.apply(Command::SetGnssRate { hz: 10 }),
            Err(CommandError::OutOfRange)
        );
        assert_eq!(
            settings.apply(Command::SetGnssRate { hz: 0 }),
            Err(CommandError::OutOfRange)
        );
        assert_eq!(
            settings.gnss_hz, 5,
            "a refused change leaves the setting alone"
        );
    }

    #[test]
    fn hdop_is_bounded_from_one_to_ten() {
        let mut settings = Settings::default();
        assert_eq!(settings.apply(Command::SetMaxHdop { tenths: 10 }), Ok(()));
        assert_eq!(settings.apply(Command::SetMaxHdop { tenths: 100 }), Ok(()));
        assert_eq!(
            settings.apply(Command::SetMaxHdop { tenths: 9 }),
            Err(CommandError::OutOfRange)
        );
        assert_eq!(
            settings.apply(Command::SetMaxHdop { tenths: 101 }),
            Err(CommandError::OutOfRange)
        );
    }

    #[test]
    fn minimum_move_is_bounded_from_one_to_fifty_metres() {
        let mut settings = Settings::default();
        assert_eq!(settings.apply(Command::SetMinMove { metres: 1 }), Ok(()));
        assert_eq!(
            settings.apply(Command::SetMinMove { metres: 0 }),
            Err(CommandError::OutOfRange)
        );
        assert_eq!(
            settings.apply(Command::SetMinMove { metres: 51 }),
            Err(CommandError::OutOfRange)
        );
    }

    #[test]
    fn settings_map_onto_the_tracker_config() {
        let settings = Settings {
            gnss_hz: 1,
            max_hdop_tenths: 25,
            min_move_m: 8,
        };
        let config = settings.tracker_config();
        assert_eq!(config.max_hdop, 2.5);
        assert_eq!(config.min_move_m, 8.0);
        assert_eq!(config.max_speed_mps, TrackerConfig::default().max_speed_mps);
    }

    #[test]
    fn responses_are_ok_or_invalid() {
        assert_eq!(response_for(Ok(())), RESP_OK);
        assert_eq!(response_for(Err(CommandError::OutOfRange)), RESP_INVALID);
    }
}
