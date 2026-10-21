// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Wire-format tests for the serde-enabled types.
//!
//! The golden file in `tests/golden/fix_v1.json` is the version 1 JSON form of
//! [`Fix`]. Changing it is a breaking change to the wire format.

#![cfg(feature = "serde")]

use serde_json::{json, Value};
use tpt_ruc_core::ruc_rates::PurchaseMethod;
use tpt_ruc_core::{Config, Coordinate, Fix, FixQuality, Liability, Segment, Vehicle};

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

#[test]
fn liability_serialises_with_named_money_fields() {
    // Output-only: the RUC models hold `'static` borrows, so they encode but do not decode.
    let liability = Liability {
        rate_version: "2026-08-06",
        billed_metres: 12_345,
        incl_gst_micros: 4_000_000,
        excl_gst_micros: 3_478_261,
        gst_micros: 521_739,
    };
    let encoded = serde_json::to_value(liability).unwrap();
    assert_eq!(
        encoded,
        json!({
            "rate_version": "2026-08-06",
            "billed_metres": 12345,
            "incl_gst_micros": 4000000,
            "excl_gst_micros": 3478261,
            "gst_micros": 521739
        })
    );
}

#[test]
fn vehicle_serialises_rate_type_and_weight() {
    let vehicle = Vehicle {
        rate_type: "H61",
        gross_kg: 44_000,
        electric: false,
    };
    let encoded = serde_json::to_value(vehicle).unwrap();
    assert_eq!(
        encoded,
        json!({ "rate_type": "H61", "gross_kg": 44000, "electric": false })
    );
}

#[test]
fn purchase_method_round_trips() {
    let encoded = serde_json::to_value(PurchaseMethod::Agent).unwrap();
    assert_eq!(encoded, json!("Agent"));
    let decoded: PurchaseMethod = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, PurchaseMethod::Agent);
}
