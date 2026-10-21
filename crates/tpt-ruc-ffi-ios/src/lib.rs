// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Swift bindings for tpt-ruc-core.
//!
//! The bridge below is turned into Swift code by swift-bridge (see
//! `build-ios.sh` and `samples/ios/`). A `Result` with a `String` error becomes
//! a throwing Swift function.
//!
//! swift-bridge can return `Result` only with floating-point values, so money
//! comes back as NZD dollars in a `Double`. Each value is exact to the micro-dollar.
//! Round it to cents for display.

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        // GST-inclusive charge for one trip, in NZD. Throws on bad input.
        fn liability_incl_gst_nzd(
            rate_type: &str,
            gross_kg: u32,
            electric: bool,
            year: u16,
            month: u8,
            day: u8,
            metres: f64,
        ) -> Result<f64, String>;

        // Distance in kilometres, rounded the way RUC bills it. Throws on bad input.
        fn billable_km(metres: f64) -> Result<f64, String>;
    }
}

fn liability_incl_gst_nzd(
    rate_type: &str,
    gross_kg: u32,
    electric: bool,
    year: u16,
    month: u8,
    day: u8,
    metres: f64,
) -> Result<f64, String> {
    let micros = tpt_ruc_ffi_common::liability_incl_gst_micros(
        rate_type, gross_kg, electric, year, month, day, metres,
    )?;
    Ok(micros as f64 / 1_000_000.0)
}

fn billable_km(metres: f64) -> Result<f64, String> {
    tpt_ruc_ffi_common::billable_km(metres)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dollars_match_the_micro_dollar_wrapper() {
        let dollars = liability_incl_gst_nzd("1", 3_000, false, 2026, 10, 1, 12_345.0).unwrap();
        let micros =
            tpt_ruc_ffi_common::liability_incl_gst_micros("1", 3_000, false, 2026, 10, 1, 12_345.0)
                .unwrap();
        assert_eq!(dollars, micros as f64 / 1_000_000.0);
    }

    #[test]
    fn errors_pass_through_as_messages() {
        assert!(liability_incl_gst_nzd("no-such", 3_000, false, 2026, 10, 1, 1.0).is_err());
        assert!(billable_km(-1.0).is_err());
    }
}
