// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Parses the NMEA fixtures in `tests/fixtures`. See the README there for how
//! to add real captures.

use tpt_ruc_core::{parse_sentence, Sentence};

/// Synthetic, not captured from a receiver. See `tests/fixtures/README.md`.
const SYNTHETIC_ONE_EPOCH: &str = include_str!("fixtures/synthetic-one-epoch.nmea");

#[test]
fn every_line_of_the_synthetic_epoch_parses() {
    let mut count = 0;
    for line in SYNTHETIC_ONE_EPOCH.lines().filter(|line| !line.is_empty()) {
        parse_sentence(line).unwrap_or_else(|error| panic!("{line}: {error:?}"));
        count += 1;
    }
    assert_eq!(count, 4, "one epoch is GGA, RMC, VTG and GSA");
}

#[test]
fn the_synthetic_gga_position_is_south_east() {
    let first = SYNTHETIC_ONE_EPOCH.lines().next().unwrap();
    let Ok(Sentence::Gga(gga)) = parse_sentence(first) else {
        panic!("first line should be GGA");
    };
    let position = gga.position.expect("GGA carries a position");
    // 41 degrees 16.7 minutes south, which is about -41.27833.
    assert!((position.lat.degrees() + 41.278_333).abs() < 1e-5);
    assert!((position.lon.degrees() - 174.776_333).abs() < 1e-5);
}

#[test]
fn a_line_with_a_wrong_checksum_is_refused() {
    let first = SYNTHETIC_ONE_EPOCH.lines().next().unwrap();
    let corrupted = first.replace("*56", "*57");
    assert!(parse_sentence(&corrupted).is_err());
}

#[test]
fn crlf_line_endings_parse_the_same_as_lf() {
    for line in SYNTHETIC_ONE_EPOCH.lines().filter(|line| !line.is_empty()) {
        let crlf = format!("{line}\r\n");
        assert_eq!(parse_sentence(&crlf), parse_sentence(line));
    }
}
