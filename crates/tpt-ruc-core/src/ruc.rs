// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! RUC liability from billable distance and a versioned rate table.
//!
//! NZTA rates are keyed by a vehicle type code, a weight band and a date.
//! Each rule is a rate in dollars per 1,000 km, including GST. Trailers are
//! licensed separately under their own type codes, so no trailer surcharge is
//! applied here.
//!
//! A [`RateSchedule`] is a slice of [`RateTable`]s sorted by effective date, so
//! a backdated trip is priced with the table in force on its date. The official
//! NZTA figures live in [`crate::ruc_rates`]. Tests here use fixture rates.
//!
//! Money is held in integer micro-dollars (`1_000_000` = $1.00). Rounding
//! happens once, on the final charge, rounding half up.

/// A calendar date used to pick the rate table in force. Field order makes
/// the derived ordering chronological.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RateDate {
    /// Full year, for example 2026.
    pub year: u16,
    /// Month, 1..=12.
    pub month: u8,
    /// Day of month, 1..=31.
    pub day: u8,
}

/// First day electric vehicles owe RUC. Earlier trips owe nothing.
pub const EV_LIABLE_FROM: RateDate = RateDate {
    year: 2024,
    month: 4,
    day: 1,
};

/// A vehicle as it is charged for RUC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Vehicle {
    /// NZTA RUC vehicle type code, for example `"1"` or `"H61"`. Chosen from the
    /// vehicle's configuration, as NZTA assigns it.
    pub rate_type: &'static str,
    /// RUC weight in kilograms. For overweight types this is the permit weight.
    pub gross_kg: u32,
    /// True for a battery electric vehicle. Such vehicles owe nothing before
    /// [`EV_LIABLE_FROM`].
    pub electric: bool,
}

/// One pricing rule. It applies when `min_gross_kg < gross_kg <= max_gross_kg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Rule {
    /// Vehicle type code this rule prices.
    pub type_code: &'static str,
    /// Lower weight bound, exclusive. Use `0` for "up to".
    pub min_gross_kg: u32,
    /// Upper weight bound, inclusive. Use `u32::MAX` for "any weight over".
    pub max_gross_kg: u32,
    /// Charge in micro-dollars per 1,000 km, including GST.
    pub rate_micros_per_1000km: u64,
}

/// A versioned set of rates in force from `effective` until the next table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RateTable {
    /// Identifier cited on every liability figure, for example a source and retrieval date.
    pub version: &'static str,
    /// First date this table applies.
    pub effective: RateDate,
    /// Pricing rules. Each type's bands must appear in ascending order.
    pub rules: &'static [Rule],
    /// GST in basis points, where 1500 is 15%.
    pub gst_basis_points: u32,
}

/// Rate tables sorted by ascending `effective` date.
pub type RateSchedule = &'static [RateTable];

/// The liability for one trip or period, with rounding already applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Liability {
    /// Version of the table used.
    pub rate_version: &'static str,
    /// Billable distance in whole metres.
    pub billed_metres: u64,
    /// Charge including GST, in micro-dollars. This is the figure NZTA publishes.
    pub incl_gst_micros: u64,
    /// Charge before GST, in micro-dollars.
    pub excl_gst_micros: u64,
    /// GST within the charge, in micro-dollars.
    pub gst_micros: u64,
}

/// Why a liability could not be calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum RucError {
    /// The vehicle has a zero weight or an empty type code.
    InvalidVehicle,
    /// The distance is negative or not a number.
    InvalidDistance,
    /// No rate table is in force on the requested date.
    NoRateTable,
    /// The table has no rule for this type and weight.
    NoMatchingRule,
    /// An intermediate amount does not fit in `u64`.
    Overflow,
}

/// Heavy vehicles are those over this gross weight in kilograms. Their
/// licences expire after a rate increase. Light licences do not.
pub const HEAVY_OVER_KG: u32 = 3_500;

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The date one calendar month after `date`, with the day clamped to the
/// length of the new month (so 31 January becomes 28 or 29 February).
pub fn one_month_after(date: RateDate) -> RateDate {
    let (year, month) = if date.month == 12 {
        (date.year + 1, 1)
    } else {
        (date.year, date.month + 1)
    };
    RateDate {
        year,
        month,
        day: date.day.min(days_in_month(year, month)),
    }
}

/// The date a heavy licence bought before a rate increase expires, or `None`
/// for a light vehicle, whose licence does not expire.
///
/// `rate_change` is the effective date of the new table. The licence expires
/// one month after it.
pub fn heavy_licence_expires_on(gross_kg: u32, rate_change: RateDate) -> Option<RateDate> {
    (gross_kg > HEAVY_OVER_KG).then(|| one_month_after(rate_change))
}

/// True when a heavy operator buying on `purchase` within one month after
/// `rate_change` must also buy an overlap licence.
pub fn needs_overlap_licence(gross_kg: u32, purchase: RateDate, rate_change: RateDate) -> bool {
    gross_kg > HEAVY_OVER_KG && purchase >= rate_change && purchase < one_month_after(rate_change)
}

/// Returns the table in force on `date`: the latest table whose effective
/// date is not after `date`.
///
/// `schedule` must be sorted by ascending `effective` date.
pub fn rate_for(schedule: RateSchedule, date: RateDate) -> Result<&'static RateTable, RucError> {
    schedule
        .iter()
        .rev()
        .find(|table| table.effective <= date)
        .ok_or(RucError::NoRateTable)
}

/// Returns the rule in `table` that prices `vehicle`.
pub fn rule_for(table: &RateTable, vehicle: &Vehicle) -> Result<&'static Rule, RucError> {
    table
        .rules
        .iter()
        .find(|rule| {
            rule.type_code == vehicle.rate_type
                && vehicle.gross_kg > rule.min_gross_kg
                && vehicle.gross_kg <= rule.max_gross_kg
        })
        .ok_or(RucError::NoMatchingRule)
}

/// Calculates liability for `metres` of distance driven by `vehicle` on `date`.
///
/// The distance is billed in whole metres. The charge is calculated including
/// GST, and GST is then split out of it, so the total matches the published
/// rate.
pub fn liability(
    schedule: RateSchedule,
    vehicle: &Vehicle,
    date: RateDate,
    metres: f64,
) -> Result<Liability, RucError> {
    if vehicle.gross_kg == 0 || vehicle.rate_type.is_empty() {
        return Err(RucError::InvalidVehicle);
    }
    if metres.is_nan() || metres < 0.0 {
        return Err(RucError::InvalidDistance);
    }
    if vehicle.electric && date < EV_LIABLE_FROM {
        return Ok(Liability {
            rate_version: "ev-not-liable-before-2024-04-01",
            billed_metres: libm::round(metres) as u64,
            incl_gst_micros: 0,
            excl_gst_micros: 0,
            gst_micros: 0,
        });
    }
    let table = rate_for(schedule, date)?;
    let rule = rule_for(table, vehicle)?;

    let billed_metres = libm::round(metres) as u64;

    // micro-dollars = metres * (micro-dollars per 1,000 km) / 1,000,000, half up.
    let incl = billed_metres
        .checked_mul(rule.rate_micros_per_1000km)
        .and_then(|product| product.checked_add(500_000))
        .ok_or(RucError::Overflow)?
        / 1_000_000;

    let (excl, gst) = split_gst(incl, table.gst_basis_points)?;
    Ok(Liability {
        rate_version: table.version,
        billed_metres,
        incl_gst_micros: incl,
        excl_gst_micros: excl,
        gst_micros: gst,
    })
}

/// Splits a GST-inclusive amount into `(excl, gst)` in micro-dollars.
///
/// `excl = incl * 10000 / (10000 + gst)`, rounded half up. GST is the remainder,
/// so the two parts always sum to `incl`.
pub fn split_gst(incl_micros: u64, gst_basis_points: u32) -> Result<(u64, u64), RucError> {
    let gst_factor = 10_000 + u64::from(gst_basis_points);
    let excl = incl_micros
        .checked_mul(10_000)
        .and_then(|product| product.checked_add(gst_factor / 2))
        .ok_or(RucError::Overflow)?
        / gst_factor;
    Ok((excl, incl_micros - excl))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixture rates for mechanics tests only. These are NOT NZTA rates.
    const FIXTURE_RULES_2025: &[Rule] = &[
        Rule {
            type_code: "1",
            min_gross_kg: 0,
            max_gross_kg: 3_500,
            rate_micros_per_1000km: 10_000_000,
        },
        Rule {
            type_code: "1",
            min_gross_kg: 3_500,
            max_gross_kg: u32::MAX,
            rate_micros_per_1000km: 20_000_000,
        },
    ];

    const FIXTURE_RULES_2026: &[Rule] = &[Rule {
        type_code: "1",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: 30_000_000,
    }];

    const SCHEDULE: RateSchedule = &[
        RateTable {
            version: "fixture-2025",
            effective: RateDate {
                year: 2025,
                month: 1,
                day: 1,
            },
            rules: FIXTURE_RULES_2025,
            gst_basis_points: 1500,
        },
        RateTable {
            version: "fixture-2026",
            effective: RateDate {
                year: 2026,
                month: 7,
                day: 1,
            },
            rules: FIXTURE_RULES_2026,
            gst_basis_points: 1500,
        },
    ];

    fn date(year: u16, month: u8, day: u8) -> RateDate {
        RateDate { year, month, day }
    }

    fn vehicle(rate_type: &'static str, gross_kg: u32) -> Vehicle {
        Vehicle {
            rate_type,
            gross_kg,
            electric: false,
        }
    }

    fn ev(rate_type: &'static str, gross_kg: u32) -> Vehicle {
        Vehicle {
            electric: true,
            ..vehicle(rate_type, gross_kg)
        }
    }

    #[test]
    fn charge_is_split_into_excl_and_gst_summing_to_incl() {
        // 100 km at $10.00 per 1,000 km is $1.00 incl GST.
        let l = liability(SCHEDULE, &vehicle("1", 2_000), date(2025, 3, 1), 100_000.0).unwrap();
        assert_eq!(l.billed_metres, 100_000);
        assert_eq!(l.incl_gst_micros, 1_000_000);
        // 1,000,000 * 10000 / 11500 = 869,565.2 rounds to 869,565.
        assert_eq!(l.excl_gst_micros, 869_565);
        assert_eq!(l.gst_micros, 130_435);
        assert_eq!(l.excl_gst_micros + l.gst_micros, l.incl_gst_micros);
        assert_eq!(l.rate_version, "fixture-2025");
    }

    #[test]
    fn weight_band_selects_the_rule() {
        let light = liability(
            SCHEDULE,
            &vehicle("1", 3_500),
            date(2025, 3, 1),
            1_000_000.0,
        );
        let heavy = liability(
            SCHEDULE,
            &vehicle("1", 3_501),
            date(2025, 3, 1),
            1_000_000.0,
        );
        // 1,000 km at $10 and at $20 per 1,000 km.
        assert_eq!(light.unwrap().incl_gst_micros, 10_000_000);
        assert_eq!(heavy.unwrap().incl_gst_micros, 20_000_000);
    }

    #[test]
    fn backdated_trip_uses_the_table_in_force_then() {
        let v = vehicle("1", 2_000);
        let old = liability(SCHEDULE, &v, date(2026, 6, 30), 1_000_000.0).unwrap();
        let new = liability(SCHEDULE, &v, date(2026, 7, 1), 1_000_000.0).unwrap();
        assert_eq!(old.rate_version, "fixture-2025");
        assert_eq!(new.rate_version, "fixture-2026");
        assert_eq!(new.incl_gst_micros, 30_000_000);
    }

    #[test]
    fn date_before_first_table_is_an_error() {
        let result = liability(SCHEDULE, &vehicle("1", 2_000), date(2024, 12, 31), 1.0);
        assert_eq!(result, Err(RucError::NoRateTable));
    }

    #[test]
    fn electric_vehicle_owes_nothing_before_ev_start_date() {
        let before = liability(SCHEDULE, &ev("1", 2_000), date(2024, 3, 31), 1_000_000.0).unwrap();
        assert_eq!(before.incl_gst_micros, 0);
        assert_eq!(before.billed_metres, 1_000_000);
    }

    #[test]
    fn electric_vehicle_is_charged_from_ev_start_date() {
        // The fixture schedule has no table before 2025, so use a 2025 date.
        let on_start = liability(SCHEDULE, &ev("1", 2_000), date(2025, 3, 1), 1_000_000.0).unwrap();
        assert_eq!(on_start.incl_gst_micros, 10_000_000);
    }

    #[test]
    fn diesel_before_ev_start_date_still_needs_a_table() {
        // Only EVs get the exemption. Other vehicles still need a table.
        let result = liability(SCHEDULE, &vehicle("1", 2_000), date(2024, 3, 31), 1.0);
        assert_eq!(result, Err(RucError::NoRateTable));
    }

    #[test]
    fn unknown_type_is_an_error() {
        let result = liability(SCHEDULE, &vehicle("999", 2_000), date(2025, 3, 1), 1.0);
        assert_eq!(result, Err(RucError::NoMatchingRule));
    }

    #[test]
    fn invalid_vehicle_and_distance_are_rejected() {
        assert_eq!(
            liability(SCHEDULE, &vehicle("1", 0), date(2025, 3, 1), 1.0),
            Err(RucError::InvalidVehicle)
        );
        assert_eq!(
            liability(SCHEDULE, &vehicle("", 2_000), date(2025, 3, 1), 1.0),
            Err(RucError::InvalidVehicle)
        );
        assert_eq!(
            liability(SCHEDULE, &vehicle("1", 2_000), date(2025, 3, 1), -1.0),
            Err(RucError::InvalidDistance)
        );
        assert_eq!(
            liability(SCHEDULE, &vehicle("1", 2_000), date(2025, 3, 1), f64::NAN),
            Err(RucError::InvalidDistance)
        );
    }

    #[test]
    fn distance_is_rounded_to_whole_metres_once() {
        // 0.4 m rounds to 0 m, so nothing is charged.
        let l = liability(SCHEDULE, &vehicle("1", 2_000), date(2025, 3, 1), 0.4).unwrap();
        assert_eq!(l.billed_metres, 0);
        assert_eq!(l.incl_gst_micros, 0);
    }

    #[test]
    fn rate_for_picks_latest_table_not_after_date() {
        assert_eq!(
            rate_for(SCHEDULE, date(2025, 1, 1)).unwrap().version,
            "fixture-2025"
        );
        assert_eq!(
            rate_for(SCHEDULE, date(2030, 1, 1)).unwrap().version,
            "fixture-2026"
        );
    }
}

#[cfg(test)]
mod licence_tests {
    use super::*;

    fn d(year: u16, month: u8, day: u8) -> RateDate {
        RateDate { year, month, day }
    }

    #[test]
    fn one_month_after_keeps_the_day_when_it_exists() {
        assert_eq!(one_month_after(d(2026, 8, 6)), d(2026, 9, 6));
    }

    #[test]
    fn one_month_after_wraps_the_year() {
        assert_eq!(one_month_after(d(2026, 12, 15)), d(2027, 1, 15));
    }

    #[test]
    fn one_month_after_clamps_to_month_length() {
        assert_eq!(one_month_after(d(2026, 1, 31)), d(2026, 2, 28));
        assert_eq!(one_month_after(d(2028, 1, 31)), d(2028, 2, 29));
        assert_eq!(one_month_after(d(2026, 8, 31)), d(2026, 9, 30));
    }

    #[test]
    fn leap_year_rules_for_february() {
        assert_eq!(one_month_after(d(1900, 1, 31)), d(1900, 2, 28));
        assert_eq!(one_month_after(d(2000, 1, 31)), d(2000, 2, 29));
    }

    #[test]
    fn heavy_licence_expires_one_month_after_rate_change() {
        assert_eq!(
            heavy_licence_expires_on(3_501, d(2026, 8, 6)),
            Some(d(2026, 9, 6))
        );
    }

    #[test]
    fn light_licence_does_not_expire() {
        assert_eq!(heavy_licence_expires_on(3_500, d(2026, 8, 6)), None);
        assert_eq!(heavy_licence_expires_on(2_000, d(2026, 8, 6)), None);
    }

    #[test]
    fn overlap_licence_is_needed_for_heavy_purchases_within_a_month() {
        let change = d(2026, 8, 6);
        assert!(needs_overlap_licence(10_000, d(2026, 8, 6), change));
        assert!(needs_overlap_licence(10_000, d(2026, 9, 5), change));
        // Exactly one month after the change, the new rate applies and no overlap is needed.
        assert!(!needs_overlap_licence(10_000, d(2026, 9, 6), change));
        // Before the change there is no overlap to buy.
        assert!(!needs_overlap_licence(10_000, d(2026, 8, 5), change));
        assert!(!needs_overlap_licence(2_000, d(2026, 8, 6), change));
    }
}
