// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! On-device distance recording: GNSS bytes in, finished segments out.
//!
//! The recorder runs the same filters and segment rules as the phone, so a
//! trip recorded on the device matches one recorded in the PWA. Finished
//! segments wait in a [`SegmentQueue`] until they are sent.

use tpt_ruc_core::{Config, Tracker};

use crate::queue::SegmentQueue;
use crate::GnssReader;

/// Records distance from a GNSS byte stream, and queues finished segments.
#[derive(Debug, Clone, Copy)]
pub struct Recorder<const N: usize> {
    reader: GnssReader,
    tracker: Tracker,
    queue: SegmentQueue<N>,
    rejected: u32,
}

impl<const N: usize> Recorder<N> {
    /// Creates a recorder with the core crate's default filter thresholds.
    pub fn new() -> Self {
        Self {
            reader: GnssReader::new(),
            tracker: Tracker::new(Config::default()),
            queue: SegmentQueue::new(),
            rejected: 0,
        }
    }

    /// Feeds one byte from the GNSS UART.
    pub fn push_byte(&mut self, byte: u8) {
        let Some(fix) = self.reader.push(byte) else {
            return;
        };
        match self.tracker.push(fix) {
            Ok(accepted) => {
                if let Some(closed) = accepted.closed {
                    self.queue.push(closed);
                }
            }
            // Every rejection leaves the distance unchanged. Only the count is kept.
            Err(_) => {
                self.rejected = self.rejected.saturating_add(1);
            }
        }
    }

    /// Closes the open segment, for example when the ignition is switched off.
    /// Consumes the recorder, so nothing more can be recorded after it.
    pub fn finish(mut self) -> SegmentQueue<N> {
        if let Some(open) = self.tracker.finish() {
            self.queue.push(open);
        }
        self.queue
    }

    /// Takes the oldest finished segment, for the BLE sender.
    pub fn next_segment(&mut self) -> Option<tpt_ruc_core::Segment> {
        self.queue.pop()
    }

    /// Total distance accepted since the recorder was created, in metres.
    pub fn total_m(&self) -> f64 {
        self.tracker.total_m()
    }

    /// Fixes the filters rejected, since the recorder was created.
    pub fn rejected(&self) -> u32 {
        self.rejected
    }

    /// Segments dropped because the queue was full.
    pub fn dropped(&self) -> u32 {
        self.queue.dropped()
    }
}

impl<const N: usize> Default for Recorder<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_ruc_core::Segment;

    /// Builds one GGA+RMC pair for a point `north_m` metres north of a start point,
    /// at `second` seconds after 2026-10-21 12:00:00 UTC. The pair is sent RMC first.
    fn pair(north_m: f64, second: u32, speed_knots: f64) -> std::string::String {
        // 1 degree of latitude is about 111,195 m. Work in ddmm.mmmm.
        let lat_deg = -41.0 + north_m / 111_195.08;
        let abs = if lat_deg < 0.0 { -lat_deg } else { lat_deg };
        let whole = abs.trunc();
        let minutes = (abs - whole) * 60.0;
        let hh = 12u32;
        let mm = second / 60;
        let ss = second % 60;
        let body_rmc = std::format!(
            "GPRMC,{hh:02}{mm:02}{ss:02}.00,A,{:02}{:08.5},S,17446.0000,E,{speed_knots:.1},0.0,211026,,,A",
            whole as u32,
            minutes
        );
        let rmc = std::format!("${body_rmc}*{:02X}\r\n", checksum(&body_rmc));
        let body_gga = std::format!(
            "GPGGA,{hh:02}{mm:02}{ss:02}.00,{:02}{:08.5},S,17446.0000,E,1,08,0.9,10.0,M,0.0,M,,",
            whole as u32,
            minutes
        );
        let gga = std::format!("${body_gga}*{:02X}\r\n", checksum(&body_gga));
        std::format!("{rmc}{gga}")
    }

    fn checksum(body: &str) -> u8 {
        body.bytes().fold(0u8, |acc, b| acc ^ b)
    }

    fn feed<const N: usize>(recorder: &mut Recorder<N>, text: &str) {
        for byte in text.bytes() {
            recorder.push_byte(byte);
        }
    }

    #[test]
    fn checksum_helper_matches_a_published_sentence() {
        // The published checksum for the classic GGA sample.
        let body = "GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        assert_eq!(checksum(body), 0x47);
    }

    #[test]
    fn records_distance_from_a_moving_stream() {
        let mut recorder: Recorder<8> = Recorder::new();
        // Each fix is 10 m north, one second apart, at about 19.4 knots (10 m/s).
        for i in 0..10u32 {
            let text = pair(10.0 * f64::from(i), i, 19.4);
            feed(&mut recorder, &text);
        }
        assert!(recorder.total_m() > 80.0, "got {}", recorder.total_m());
        assert!(recorder.total_m() < 110.0, "got {}", recorder.total_m());
    }

    #[test]
    fn parked_stream_records_nothing() {
        let mut recorder: Recorder<8> = Recorder::new();
        for i in 0..10u32 {
            feed(&mut recorder, &pair(0.0, i, 0.0));
        }
        assert_eq!(recorder.total_m(), 0.0);
    }

    #[test]
    fn finish_returns_the_open_segment() {
        let mut recorder: Recorder<8> = Recorder::new();
        for i in 0..5u32 {
            feed(&mut recorder, &pair(10.0 * f64::from(i), i, 19.4));
        }
        let mut queue = recorder.finish();
        let segment: Segment = queue.pop().expect("open segment should be queued");
        assert!(segment.distance_m > 0.0);
    }
}
