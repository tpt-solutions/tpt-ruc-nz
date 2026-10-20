// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Property tests: arbitrary input must never panic, and the tracker's total
//! must only ever grow.

use proptest::prelude::*;
use tpt_ruc_core::{parse_sentence, Config, Coordinate, Fix, FixQuality, Tracker};

fn fix_quality() -> impl Strategy<Value = FixQuality> {
    prop_oneof![
        Just(FixQuality::NoFix),
        Just(FixQuality::Gps),
        Just(FixQuality::Dgps),
        Just(FixQuality::RtkFixed),
        Just(FixQuality::Estimated),
        Just(FixQuality::Simulation),
    ]
}

fn fix() -> impl Strategy<Value = Fix> {
    (
        -90.0..=90.0f64,
        -180.0..=180.0f64,
        0u64..10_000_000,
        fix_quality(),
        proptest::option::of(0.0..50.0f64),
        proptest::option::of(0.0..100.0f64),
    )
        .prop_map(|(lat, lon, time_ms, fix, hdop, speed_mps)| Fix {
            coord: Coordinate::new(lat, lon).unwrap(),
            time_ms,
            fix,
            hdop,
            speed_mps,
        })
}

proptest! {
    #[test]
    fn parser_never_panics_on_arbitrary_text(line in ".*") {
        let _ = parse_sentence(&line);
    }

    #[test]
    fn parser_never_panics_on_nmea_shaped_text(
        body in "[$A-Za-z0-9,.*+-]{0,120}",
    ) {
        let _ = parse_sentence(&body);
    }

    #[test]
    fn tracker_total_never_decreases_and_stays_finite(
        fixes in proptest::collection::vec(fix(), 0..200),
    ) {
        let mut tracker = Tracker::new(Config::default());
        let mut last_total = 0.0;
        for f in fixes {
            let _ = tracker.push(f);
            let total = tracker.total_m();
            prop_assert!(total.is_finite());
            prop_assert!(total >= last_total);
            last_total = total;
        }
    }
}
