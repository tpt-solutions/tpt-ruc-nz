// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Quality filtering and trip segment accumulation.
//!
//! [`Tracker`] consumes a stream of [`Fix`] samples, one per receiver epoch,
//! and decides which ones add distance. It rejects samples with no fix, poor
//! HDOP, impossible jumps and stationary drift. It splits a trip into
//! [`Segment`]s when the signal is lost for longer than `max_gap_ms`.
//!
//! Distances are accumulated in metres. Convert to billable kilometres only
//! once, on the final total, with [`billable_km`].

use crate::coord::Coordinate;
use crate::distance::haversine_m;
use crate::nmea::FixQuality;

/// One receiver epoch, ready for filtering.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fix {
    /// Position of the epoch.
    pub coord: Coordinate,
    /// Timestamp in milliseconds. Any monotonic clock works, such as Unix time.
    pub time_ms: u64,
    /// Fix quality reported by the receiver.
    pub fix: FixQuality,
    /// Horizontal dilution of precision, if reported.
    pub hdop: Option<f64>,
    /// Speed over ground in metres per second, if reported.
    pub speed_mps: Option<f64>,
}

/// Thresholds for the quality filters and segment splitting.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Config {
    /// Samples with HDOP above this value are rejected.
    pub max_hdop: f64,
    /// Implied speed between accepted samples above this value is a jump.
    pub max_speed_mps: f64,
    /// Below this reported speed the receiver is treated as stationary.
    pub stationary_speed_mps: f64,
    /// While stationary, displacement below this distance is drift and is ignored.
    pub min_move_m: f64,
    /// A gap in valid samples longer than this closes the current segment.
    pub max_gap_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_hdop: 5.0,
            // About 216 km/h. Well above any road vehicle in New Zealand.
            max_speed_mps: 60.0,
            stationary_speed_mps: 0.5,
            min_move_m: 5.0,
            max_gap_ms: 30_000,
        }
    }
}

/// A continuous stretch of accepted movement.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Segment {
    /// Time of the first sample in the segment, in milliseconds.
    pub start_ms: u64,
    /// Time of the last accepted sample in the segment, in milliseconds.
    pub end_ms: u64,
    /// Distance accumulated in the segment, in metres.
    pub distance_m: f64,
}

/// Why a sample was not used for distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// The receiver reports no usable position.
    NoFix,
    /// HDOP is above `max_hdop`.
    Hdop,
    /// Timestamp is not later than the previous valid sample.
    NonIncreasingTime,
    /// The implied speed from the last moving sample is above `max_speed_mps`.
    Jump,
    /// The receiver is stationary and the sample is within `min_move_m` of the last moving sample.
    Stationary,
}

/// A sample that was accepted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Accepted {
    /// Distance added by this sample, in metres.
    pub metres: f64,
    /// Segment closed by a gap before this sample, if any.
    pub closed: Option<Segment>,
}

/// Stateful filter and accumulator for one trip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tracker {
    config: Config,
    /// Last sample that moved the vehicle. Distances are measured from here.
    anchor: Option<Fix>,
    /// Time of the last sample that passed the quality checks.
    last_seen_ms: Option<u64>,
    open: Option<Segment>,
    total_m: f64,
}

impl Tracker {
    /// Creates a tracker with the given thresholds.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            anchor: None,
            last_seen_ms: None,
            open: None,
            total_m: 0.0,
        }
    }

    /// Total distance accepted so far, in metres, across all segments.
    pub fn total_m(&self) -> f64 {
        self.total_m
    }

    /// Feeds one sample through the filters.
    ///
    /// Returns the distance it added, and any segment it closed. Rejected
    /// samples leave the total unchanged.
    pub fn push(&mut self, fix: Fix) -> Result<Accepted, Rejection> {
        if !fix.fix.has_position() {
            return Err(Rejection::NoFix);
        }
        if fix.hdop.is_some_and(|hdop| hdop > self.config.max_hdop) {
            return Err(Rejection::Hdop);
        }
        if let Some(seen) = self.last_seen_ms {
            if fix.time_ms <= seen {
                return Err(Rejection::NonIncreasingTime);
            }
        }

        let gap = self
            .last_seen_ms
            .is_none_or(|seen| fix.time_ms - seen > self.config.max_gap_ms);
        let anchor = match (gap, self.anchor) {
            (false, Some(anchor)) => anchor,
            _ => return Ok(self.start_segment(fix)),
        };

        let metres = haversine_m(anchor.coord, fix.coord);
        let elapsed_ms = fix.time_ms - anchor.time_ms;
        if metres * 1000.0 > self.config.max_speed_mps * elapsed_ms as f64 {
            return Err(Rejection::Jump);
        }

        self.last_seen_ms = Some(fix.time_ms);
        let stationary = fix
            .speed_mps
            .is_some_and(|speed| speed < self.config.stationary_speed_mps);
        if stationary && metres < self.config.min_move_m {
            return Err(Rejection::Stationary);
        }

        self.anchor = Some(fix);
        self.total_m += metres;
        if let Some(open) = self.open.as_mut() {
            open.distance_m += metres;
            open.end_ms = fix.time_ms;
        }
        Ok(Accepted {
            metres,
            closed: None,
        })
    }

    /// Closes the open segment, if any, at the end of the trip.
    pub fn finish(mut self) -> Option<Segment> {
        self.open.take()
    }

    /// Closes the open segment, if any, and starts a new one at `fix`.
    fn start_segment(&mut self, fix: Fix) -> Accepted {
        let closed = self.open.take();
        self.anchor = Some(fix);
        self.last_seen_ms = Some(fix.time_ms);
        self.open = Some(Segment {
            start_ms: fix.time_ms,
            end_ms: fix.time_ms,
            distance_m: 0.0,
        });
        Accepted {
            metres: 0.0,
            closed,
        }
    }
}

/// Converts an accumulated distance in metres to billable kilometres.
///
/// Rounds to the nearest whole metre, then expresses the result in km with
/// three decimal places. Call it on the trip or period total, never on each
/// sample or segment, so rounding error does not build up.
pub fn billable_km(metres: f64) -> f64 {
    libm::round(metres) / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPOCH_MS: u64 = 1_700_000_000_000;

    /// Metres per degree of latitude on the mean-radius sphere.
    const M_PER_DEG_LAT: f64 = 111_195.08;

    fn fix_at(lat: f64, t_ms: u64) -> Fix {
        Fix {
            coord: Coordinate::new(lat, 174.0).unwrap(),
            time_ms: EPOCH_MS + t_ms,
            fix: FixQuality::Gps,
            hdop: Some(0.9),
            speed_mps: None,
        }
    }

    /// Latitude offset in degrees for a northward displacement of `metres`.
    fn north(metres: f64) -> f64 {
        metres / M_PER_DEG_LAT
    }

    fn tracker() -> Tracker {
        Tracker::new(Config::default())
    }

    #[test]
    fn accumulates_steady_movement() {
        let mut t = tracker();
        t.push(fix_at(-41.0, 0)).unwrap();
        for i in 1..=10 {
            let accepted = t
                .push(fix_at(-41.0 + north(10.0 * i as f64), i * 1000))
                .unwrap();
            assert!(
                (accepted.metres - 10.0).abs() < 0.01,
                "got {}",
                accepted.metres
            );
        }
        assert!((t.total_m() - 100.0).abs() < 0.1, "got {}", t.total_m());
    }

    #[test]
    fn rejects_samples_without_a_fix() {
        let mut t = tracker();
        let mut fix = fix_at(-41.0, 0);
        fix.fix = FixQuality::NoFix;
        assert_eq!(t.push(fix), Err(Rejection::NoFix));
    }

    #[test]
    fn rejects_high_hdop() {
        let mut t = tracker();
        let mut fix = fix_at(-41.0, 0);
        fix.hdop = Some(12.0);
        assert_eq!(t.push(fix), Err(Rejection::Hdop));
    }

    #[test]
    fn rejects_position_jumps() {
        let mut t = tracker();
        t.push(fix_at(-41.0, 0)).unwrap();
        // About 11 km in one second.
        let result = t.push(fix_at(-41.1, 1000));
        assert_eq!(result, Err(Rejection::Jump));
        assert_eq!(t.total_m(), 0.0);
    }

    #[test]
    fn rejects_non_increasing_time() {
        let mut t = tracker();
        t.push(fix_at(-41.0, 1000)).unwrap();
        assert_eq!(
            t.push(fix_at(-41.0, 1000)),
            Err(Rejection::NonIncreasingTime)
        );
        assert_eq!(
            t.push(fix_at(-41.0, 500)),
            Err(Rejection::NonIncreasingTime)
        );
    }

    #[test]
    fn ignores_stationary_drift() {
        let mut t = tracker();
        let mut first = fix_at(-41.0, 0);
        first.speed_mps = Some(0.0);
        t.push(first).unwrap();
        // Jitter of about 2 m while parked, reported at zero speed.
        for i in 1..=60 {
            let jitter = if i % 2 == 0 { 2.0 } else { -2.0 };
            let mut fix = fix_at(-41.0 + north(jitter), i * 1000);
            fix.speed_mps = Some(0.0);
            assert_eq!(t.push(fix), Err(Rejection::Stationary));
        }
        assert_eq!(t.total_m(), 0.0);
    }

    #[test]
    fn slow_creep_past_threshold_is_counted() {
        let mut t = tracker();
        let mut first = fix_at(-41.0, 0);
        first.speed_mps = Some(0.0);
        t.push(first).unwrap();
        // Still reported as slow, but now 8 m from the anchor.
        let mut fix = fix_at(-41.0 + north(8.0), 1000);
        fix.speed_mps = Some(0.3);
        let accepted = t.push(fix).unwrap();
        assert!((accepted.metres - 8.0).abs() < 0.01);
    }

    #[test]
    fn gap_closes_segment_and_starts_a_new_one() {
        let mut t = tracker();
        t.push(fix_at(-41.0, 0)).unwrap();
        // 100 m in 10 s is 10 m/s, well under the jump limit.
        t.push(fix_at(-41.0 + north(100.0), 10_000)).unwrap();

        // 40 s with no samples, longer than the 30 s default gap.
        let accepted = t.push(fix_at(-41.0 + north(200.0), 50_000)).unwrap();
        let closed = accepted.closed.expect("gap should close a segment");
        assert_eq!(closed.start_ms, EPOCH_MS);
        assert_eq!(closed.end_ms, EPOCH_MS + 10_000);
        assert!((closed.distance_m - 100.0).abs() < 0.01);
        assert_eq!(accepted.metres, 0.0);

        // The jump across the gap is not counted as distance.
        assert!((t.total_m() - 100.0).abs() < 0.01);

        let last = t.finish().expect("open segment");
        assert_eq!(last.start_ms, EPOCH_MS + 50_000);
        assert_eq!(last.distance_m, 0.0);
    }

    #[test]
    fn finish_returns_open_segment() {
        let t = tracker();
        assert_eq!(t.finish(), None);

        let mut t = tracker();
        t.push(fix_at(-41.0, 0)).unwrap();
        t.push(fix_at(-41.0 + north(50.0), 1000)).unwrap();
        let segment = t.finish().unwrap();
        assert_eq!(segment.start_ms, EPOCH_MS);
        assert_eq!(segment.end_ms, EPOCH_MS + 1000);
        assert!((segment.distance_m - 50.0).abs() < 0.01);
    }

    #[test]
    fn billable_km_rounds_total_to_whole_metres() {
        assert_eq!(billable_km(1234.4), 1.234);
        assert_eq!(billable_km(1234.6), 1.235);
        assert_eq!(billable_km(0.0), 0.0);
    }
}
