// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Wire-format tests for the serde-enabled types.
//!
//! The golden file in `tests/golden/fix_v1.json` is the version 1 JSON form of
//! [`Fix`]. Changing it is a breaking change to the wire format.

#![cfg(feature = "serde")]

use serde_json::{json, Value};
use tpt_ruc_core::{Config, Coordinate, Fix, FixQuality, Segment};

const GOLDEN_FIX_V1: &str = include_str!("golden/fix_v1.json");

fn golden_fix() -> Fix {
    Fix {
        coord: Coordinate::new(-41.2865, 174.7762).unwrap(),
        time_ms: 1_700_000_000_000,
        fix: FixQuality::Gps,
        hdop: Some(0.9),
        speed_mps: Some(12.5),
    }
}

#[test]
fn fix_json_matches_golden_file() {
    let golden: Value = serde_json::from_str(GOLDEN_FIX_V1).unwrap();
    let encoded: Value = serde_json::to_value(golden_fix()).unwrap();
    assert_eq!(encoded, golden);
}

#[test]
fn fix_decodes_from_golden_json() {
    let decoded: Fix = serde_json::from_str(GOLDEN_FIX_V1).unwrap();
    assert_eq!(decoded, golden_fix());
}

#[test]
fn fix_msgpack_round_trips() {
    let bytes = rmp_serde::to_vec_named(&golden_fix()).unwrap();
    let decoded: Fix = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(decoded, golden_fix());
}

#[test]
fn out_of_range_latitude_is_rejected_on_decode() {
    let bad = json!({
        "coord": { "lat": 91.0, "lon": 174.0 },
        "time_ms": 1,
        "fix": "Gps",
        "hdop": null,
        "speed_mps": null
    });
    assert!(serde_json::from_value::<Fix>(bad).is_err());
}

#[test]
fn missing_optional_fields_decode_as_none() {
    // Receivers often omit HDOP or speed, so JSON input may leave them out.
    let partial = json!({
        "coord": { "lat": -41.0, "lon": 174.0 },
        "time_ms": 1,
        "fix": "Gps"
    });
    let fix: Fix = serde_json::from_value(partial).unwrap();
    assert_eq!(fix.hdop, None);
    assert_eq!(fix.speed_mps, None);
}

#[test]
fn config_fills_missing_keys_from_defaults() {
    let config: Config = serde_json::from_str(r#"{ "max_hdop": 2.5 }"#).unwrap();
    assert_eq!(config.max_hdop, 2.5);
    assert_eq!(
        config,
        Config {
            max_hdop: 2.5,
            ..Config::default()
        }
    );
}

#[test]
fn segment_msgpack_round_trips() {
    let segment = Segment {
        start_ms: 1,
        end_ms: 2,
        distance_m: 3.5,
    };
    let bytes = rmp_serde::to_vec_named(&segment).unwrap();
    let decoded: Segment = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(decoded, segment);
}
