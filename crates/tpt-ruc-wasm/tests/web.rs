// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Tests that run on wasm32 through Node. Run with `wasm-pack test --node`.

#![cfg(target_arch = "wasm32")]

use tpt_ruc_wasm::{
    admin_fee_incl_gst_micros, billable_km_js, current_rate_effective, haversine_m,
    heavy_licence_expires_on_js, liability, parse_nmea,
};
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn haversine_matches_core_fixture() {
    // Wellington to Auckland, about 490 km.
    let d = haversine_m(-41.2865, 174.7762, -36.8485, 174.7633).unwrap() / 1000.0;
    assert!((d - 490.0).abs() < 5.0, "got {d}");
}

#[wasm_bindgen_test]
fn haversine_rejects_out_of_range_latitude() {
    assert!(haversine_m(91.0, 0.0, 0.0, 0.0).is_err());
}

#[wasm_bindgen_test]
fn billable_km_rounds_to_whole_metres() {
    assert_eq!(billable_km_js(1234.4), 1.234);
}

#[wasm_bindgen_test]
fn admin_fees_are_available() {
    assert_eq!(admin_fee_incl_gst_micros(false), 12_440_000);
    assert_eq!(admin_fee_incl_gst_micros(true), 13_710_000);
}

#[wasm_bindgen_test]
fn liability_rejects_unknown_type_code() {
    assert!(liability("NOPE", 2_000, false, 2026, 10, 21, 1000.0).is_err());
}

#[wasm_bindgen_test]
fn liability_is_available_for_known_type() {
    assert!(liability("1", 2_000, false, 2026, 10, 21, 1000.0).is_ok());
}

#[wasm_bindgen_test]
fn parse_nmea_rejects_bad_checksum() {
    assert!(
        parse_nmea("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*00").is_err()
    );
}

#[wasm_bindgen_test]
fn heavy_licence_expiry_matches_the_core_rule() {
    let effective = current_rate_effective();
    assert_eq!(effective, "2026-08-06");
    assert_eq!(
        heavy_licence_expires_on_js(3_501, 2026, 8, 6),
        Some("2026-09-06".to_owned())
    );
    assert_eq!(heavy_licence_expires_on_js(3_500, 2026, 8, 6), None);
}
