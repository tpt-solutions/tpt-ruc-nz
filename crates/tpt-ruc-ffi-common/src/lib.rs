// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The small API that the iOS and Android bindings expose.
//!
//! It takes plain values (strings, integers, floats) so that Swift and Kotlin
//! need no knowledge of the core's types. Errors are returned as text, which
//! each binding turns into its own error type.

use tpt_ruc_core::ruc::{liability, RateDate, RucError, Vehicle};
use tpt_ruc_core::ruc_rates::NZTA_SCHEDULE;

/// The GST-inclusive charge for one trip, in micro-dollars (1,000,000 is $1.00).
///
/// * `rate_type`: NZTA vehicle type code, for example `"1"` or `"H61"`.
/// * `gross_kg`: RUC weight in kilograms.
/// * `electric`: true for a battery electric vehicle.
/// * `year`, `month`, `day`: the trip date. Rates are picked by this date.
/// * `metres`: distance driven, in metres.
pub fn liability_incl_gst_micros(
    rate_type: &str,
    gross_kg: u32,
    electric: bool,
    year: u16,
    month: u8,
    day: u8,
    metres: f64,
) -> Result<u64, String> {
    let date = RateDate { year, month, day };
    if !is_real_date(date) {
        return Err(format!("{year:04}-{month:02}-{day:02} is not a real date"));
    }
    let rate_type = known_type_code(rate_type)
        .ok_or_else(|| format!("unknown vehicle type code {rate_type}"))?;
    let vehicle = Vehicle {
        rate_type,
        gross_kg,
        electric,
    };
    liability(NZTA_SCHEDULE, &vehicle, date, metres)
        .map(|charge| charge.incl_gst_micros)
        .map_err(describe)
}

/// Distance in whole kilometres, rounded the way RUC bills it.
pub fn billable_km(metres: f64) -> Result<f64, String> {
    if !metres.is_finite() || metres < 0.0 {
        return Err("metres must be a non-negative number".to_owned());
    }
    Ok(tpt_ruc_core::billable_km(metres))
}

/// Returns the table's own copy of a type code, so the core can borrow it for `'static`.
fn known_type_code(code: &str) -> Option<&'static str> {
    NZTA_SCHEDULE
        .iter()
        .flat_map(|table| table.rules.iter())
        .find(|rule| rule.type_code == code)
        .map(|rule| rule.type_code)
}

fn describe(error: RucError) -> String {
    match error {
        RucError::InvalidVehicle => "the vehicle type or weight is not valid".to_owned(),
        RucError::InvalidDistance => "metres must be a non-negative number".to_owned(),
        RucError::NoRateTable => "no RUC rates are in force on that date".to_owned(),
        RucError::NoMatchingRule => "no RUC rate matches this vehicle type and weight".to_owned(),
        RucError::Overflow => "the charge is too large to calculate".to_owned(),
    }
}

fn is_real_date(date: RateDate) -> bool {
    let leap = (date.year.is_multiple_of(4) && !date.year.is_multiple_of(100))
        || date.year.is_multiple_of(400);
    let days = match date.month {
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    (1..=days).contains(&date.day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_core_calculation() {
        let vehicle = Vehicle {
            rate_type: "1",
            gross_kg: 3_000,
            electric: false,
        };
        let date = RateDate {
            year: 2026,
            month: 10,
            day: 1,
        };
        let expected = liability(NZTA_SCHEDULE, &vehicle, date, 12_345.0)
            .unwrap()
            .incl_gst_micros;
        assert_eq!(
            liability_incl_gst_micros("1", 3_000, false, 2026, 10, 1, 12_345.0).unwrap(),
            expected
        );
    }

    #[test]
    fn bad_inputs_come_back_as_messages() {
        assert!(liability_incl_gst_micros("no-such", 3_000, false, 2026, 10, 1, 1.0).is_err());
        assert!(liability_incl_gst_micros("1", 3_000, false, 2026, 2, 30, 1.0).is_err());
        assert!(liability_incl_gst_micros("1", 3_000, false, 2020, 1, 1, 1.0).is_err());
        assert!(liability_incl_gst_micros("1", 3_000, false, 2026, 10, 1, -1.0).is_err());
    }

    #[test]
    fn leap_days_are_checked() {
        assert!(is_real_date(RateDate {
            year: 2024,
            month: 2,
            day: 29
        }));
        assert!(!is_real_date(RateDate {
            year: 2026,
            month: 2,
            day: 29
        }));
    }

    #[test]
    fn billable_km_refuses_negative_and_nan() {
        assert!(billable_km(-1.0).is_err());
        assert!(billable_km(f64::NAN).is_err());
        assert_eq!(
            billable_km(12_345.0).unwrap(),
            tpt_ruc_core::billable_km(12_345.0)
        );
    }
}
