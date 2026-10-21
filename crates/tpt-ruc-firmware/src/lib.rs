// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Device logic for the tpt-ruc-nz tracker, kept free of hardware code.
//!
//! This crate turns a raw GNSS UART byte stream into [`Fix`] samples for
//! `tpt-ruc-core`. It has no allocation and builds for the host, so it can be
//! tested here. The ESP-IDF or esp-hal UART driver, the flash log and the BLE
//! service are not in this crate yet. They are described in `hardware/README.md`.

#![cfg_attr(not(test), no_std)]
#![warn(missing_docs)]

pub mod payload;
pub mod queue;
pub mod recorder;

pub use queue::SegmentQueue;
pub use recorder::Recorder;

#[cfg(any(
    all(feature = "esp32s3", feature = "esp32c3"),
    all(feature = "esp32s3", feature = "esp32c6"),
    all(feature = "esp32c3", feature = "esp32c6"),
    all(feature = "esp32s3", feature = "esp32p4"),
    all(feature = "esp32c3", feature = "esp32p4"),
    all(feature = "esp32c6", feature = "esp32p4"),
))]
compile_error!("enable only one chip feature: esp32s3, esp32c3, esp32c6 or esp32p4");

/// The chip this build targets, or `"host"` when no chip feature is enabled.
pub const CHIP: &str = if cfg!(feature = "esp32s3") {
    "esp32s3"
} else if cfg!(feature = "esp32c3") {
    "esp32c3"
} else if cfg!(feature = "esp32c6") {
    "esp32c6"
} else if cfg!(feature = "esp32p4") {
    "esp32p4"
} else {
    "host"
};

use tpt_ruc_core::nmea::{Date, UtcTime};
use tpt_ruc_core::{parse_sentence, Coordinate, Fix, Sentence};

/// Longest sentence kept. NMEA 0183 caps a sentence at 82 characters.
pub const MAX_SENTENCE: usize = 96;

/// Metres per second in one knot.
const MPS_PER_KNOT: f64 = 0.514_444;

/// Accumulates bytes into one NMEA line. Lines that overflow are discarded.
#[derive(Debug, Clone, Copy)]
pub struct LineBuffer {
    buf: [u8; MAX_SENTENCE],
    len: usize,
    overflowed: bool,
}

impl Default for LineBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl LineBuffer {
    /// Creates an empty buffer.
    pub const fn new() -> Self {
        Self {
            buf: [0; MAX_SENTENCE],
            len: 0,
            overflowed: false,
        }
    }

    /// Appends a byte. Bytes past the limit mark the line as overflowed.
    pub fn push(&mut self, byte: u8) {
        if self.len < MAX_SENTENCE {
            self.buf[self.len] = byte;
            self.len += 1;
        } else {
            self.overflowed = true;
        }
    }

    /// The buffered line as text, or `None` if it overflowed or is not UTF-8.
    pub fn as_str(&self) -> Option<&str> {
        if self.overflowed {
            return None;
        }
        core::str::from_utf8(&self.buf[..self.len]).ok()
    }

    /// Empties the buffer for the next line.
    pub fn clear(&mut self) {
        self.len = 0;
        self.overflowed = false;
    }
}

/// Turns a GNSS byte stream into [`Fix`] samples.
///
/// GGA carries time, position, fix quality and HDOP, but no date. RMC carries
/// the date and speed. The reader keeps the latest RMC values and merges them
/// into each GGA, so the RMC must arrive first for its date to be used.
#[derive(Debug, Clone, Copy, Default)]
pub struct GnssReader {
    line: LineBuffer,
    date: Option<Date>,
    speed_mps: Option<f64>,
}

impl GnssReader {
    /// Creates a reader with no date or speed yet.
    pub const fn new() -> Self {
        Self {
            line: LineBuffer::new(),
            date: None,
            speed_mps: None,
        }
    }

    /// Feeds one byte. Returns a fix when the byte completes a GGA line that can be merged.
    pub fn push(&mut self, byte: u8) -> Option<Fix> {
        match byte {
            b'\r' => None,
            b'\n' => self.finish_line(),
            _ => {
                self.line.push(byte);
                None
            }
        }
    }

    fn finish_line(&mut self) -> Option<Fix> {
        // Parse first, then clear, so the buffer is reset even when the line is unusable.
        let parsed = self.line.as_str().map(parse_sentence);
        self.line.clear();
        match parsed?.ok()? {
            Sentence::Rmc(rmc) => {
                self.date = rmc.date;
                self.speed_mps = rmc.speed_knots.map(|knots| knots * MPS_PER_KNOT);
                None
            }
            Sentence::Gga(gga) => {
                let coord: Coordinate = gga.position?;
                let time_ms = unix_ms(self.date?, gga.time?)?;
                Some(Fix {
                    coord,
                    time_ms,
                    fix: gga.fix,
                    hdop: gga.hdop,
                    speed_mps: self.speed_mps,
                })
            }
            _ => None,
        }
    }
}

/// Converts a UTC date and time to Unix milliseconds. Returns `None` before 1970.
pub fn unix_ms(date: Date, time: UtcTime) -> Option<u64> {
    let days = days_from_civil(
        i64::from(date.year),
        i64::from(date.month),
        i64::from(date.day),
    );
    if days < 0 {
        return None;
    }
    let seconds_of_day = i64::from(time.hour) * 3600 + i64::from(time.minute) * 60;
    let millis_in_day = libm::round(time.second * 1000.0) as u64;
    let whole = (days as u64 * 86_400 + seconds_of_day as u64) * 1000;
    Some(whole + millis_in_day)
}

/// Days from 1970-01-01 to the given civil date (proleptic Gregorian).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_ruc_core::FixQuality;

    /// Classic NMEA 0183 sample sentences, with their published checksums.
    const RMC: &str = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A";
    const GGA: &str = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47";

    fn feed(reader: &mut GnssReader, text: &str) -> Option<Fix> {
        let mut last = None;
        for byte in text.bytes() {
            if let Some(fix) = reader.push(byte) {
                last = Some(fix);
            }
        }
        last
    }

    #[test]
    fn converts_known_date_and_time() {
        let date = Date {
            day: 23,
            month: 3,
            year: 1994,
        };
        let time = UtcTime {
            hour: 12,
            minute: 35,
            second: 19.0,
        };
        assert_eq!(unix_ms(date, time), Some(764_426_119_000));
    }

    #[test]
    fn keeps_milliseconds() {
        let date = Date {
            day: 1,
            month: 1,
            year: 1970,
        };
        let time = UtcTime {
            hour: 0,
            minute: 0,
            second: 1.25,
        };
        assert_eq!(unix_ms(date, time), Some(1_250));
    }

    #[test]
    fn rejects_dates_before_1970() {
        let date = Date {
            day: 1,
            month: 1,
            year: 1969,
        };
        let time = UtcTime {
            hour: 0,
            minute: 0,
            second: 0.0,
        };
        assert_eq!(unix_ms(date, time), None);
    }

    #[test]
    fn merges_rmc_date_and_speed_into_gga() {
        let mut reader = GnssReader::new();
        let fix =
            feed(&mut reader, &format!("{RMC}\r\n{GGA}\r\n")).expect("GGA should yield a fix");

        assert_eq!(fix.time_ms, 764_426_119_000);
        assert_eq!(fix.fix, FixQuality::Gps);
        assert_eq!(fix.hdop, Some(0.9));
        // 022.4 knots is about 11.53 m/s.
        assert!((fix.speed_mps.unwrap() - 22.4 * MPS_PER_KNOT).abs() < 1e-9);
        // 4807.038 N and 01131.000 E.
        assert!((fix.coord.lat.degrees() - (48.0 + 7.038 / 60.0)).abs() < 1e-9);
        assert!((fix.coord.lon.degrees() - (11.0 + 31.0 / 60.0)).abs() < 1e-9);
    }

    #[test]
    fn gga_without_a_prior_rmc_yields_no_fix() {
        let mut reader = GnssReader::new();
        assert_eq!(feed(&mut reader, &format!("{GGA}\r\n")), None);
    }

    #[test]
    fn corrupt_checksum_is_ignored() {
        let mut reader = GnssReader::new();
        let bad = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*00";
        assert_eq!(feed(&mut reader, &format!("{bad}\r\n{GGA}\r\n")), None);
    }

    #[test]
    fn overlong_line_is_discarded_and_the_next_line_still_parses() {
        let mut reader = GnssReader::new();
        let long = format!("$GPXXX,{}*00", "1".repeat(200));
        let fix = feed(&mut reader, &format!("{long}\r\n{RMC}\r\n{GGA}\r\n"));
        assert!(fix.is_some());
    }

    #[test]
    fn line_buffer_reports_overflow() {
        let mut line = LineBuffer::new();
        for _ in 0..=MAX_SENTENCE {
            line.push(b'a');
        }
        assert_eq!(line.as_str(), None);
        line.clear();
        line.push(b'a');
        assert_eq!(line.as_str(), Some("a"));
    }
}
