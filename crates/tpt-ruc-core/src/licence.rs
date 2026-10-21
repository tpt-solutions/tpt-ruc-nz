// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Distance licence models: odometer expiry, overlap licences, refunds and
//! the Section 43 gate for exempt distance.
//!
//! Each function here is a model of one part of the scheme. A function marked
//! **requires verification** encodes a rule that has not been confirmed
//! against NZTA guidance, so it must not be used for licensing or refunds
//! until it has been checked. The model is a tool for planning and
//! estimation, not an NZTA-approved distance recorder.

use crate::ruc::{heavy_licence_expires_on, needs_overlap_licence, RateDate};

/// A purchased distance licence. It is valid for `licensed_km` driven after
/// the odometer reads `start_odometer_km` at purchase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DistanceLicence {
    /// Odometer reading at purchase, in whole kilometres.
    pub start_odometer_km: u64,
    /// Distance the licence covers, in whole kilometres.
    pub licensed_km: u64,
}

impl DistanceLicence {
    /// The odometer reading at which the licence runs out.
    pub fn expiry_odometer_km(&self) -> u64 {
        self.start_odometer_km.saturating_add(self.licensed_km)
    }

    /// Kilometres left before expiry at `odometer_km`. Zero once it is used up.
    pub fn remaining_km(&self, odometer_km: u64) -> u64 {
        self.expiry_odometer_km().saturating_sub(odometer_km)
    }
}

/// Whether a licence is still usable on a given day and odometer reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenceStatus {
    /// Still usable, with this many kilometres left.
    Valid {
        /// Kilometres left before the licence runs out.
        remaining_km: u64,
    },
    /// The licence distance has been driven.
    UsedUp,
    /// The licence date limit has passed, such as the expiry after a rate change.
    ExpiredByDate,
}

/// Works out whether `licence` is still usable.
///
/// `date_limit` is an optional calendar limit, such as the one set by
/// [`heavy_licence_expires_on`]. It is checked after the distance, so a used-up
/// licence reports `UsedUp` even when the date has also passed.
pub fn status(
    licence: &DistanceLicence,
    odometer_km: u64,
    today: RateDate,
    date_limit: Option<RateDate>,
) -> LicenceStatus {
    let remaining_km = licence.remaining_km(odometer_km);
    if remaining_km == 0 {
        LicenceStatus::UsedUp
    } else if date_limit.is_some_and(|limit| today > limit) {
        LicenceStatus::ExpiredByDate
    } else {
        LicenceStatus::Valid { remaining_km }
    }
}

/// Whole days from `from` to `to`. Negative when `to` is earlier.
///
/// Uses the proleptic Gregorian calendar, so leap years are counted.
pub fn days_between(from: RateDate, to: RateDate) -> i64 {
    days_from_civil(to) - days_from_civil(from)
}

/// Days since 1970-01-01, from Howard Hinnant's civil-date algorithm.
fn days_from_civil(date: RateDate) -> i64 {
    let year = i64::from(date.year) - i64::from(date.month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(date.month);
    let day_of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(date.day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Kilometres a heavy operator is modelled to drive on an overlap licence.
///
/// The overlap runs from the purchase date to the date the older heavy
/// licence expires, at `km_per_day` on average. It is zero unless
/// [`needs_overlap_licence`] applies.
///
/// **Requires verification.** The overlap rule and the daily distance are
/// modelled, not taken from NZTA guidance. `km_per_day` is an estimate the
/// caller must supply.
pub fn overlap_licence_km(
    gross_kg: u32,
    purchase: RateDate,
    rate_change: RateDate,
    km_per_day: u32,
) -> u64 {
    if !needs_overlap_licence(gross_kg, purchase, rate_change) {
        return 0;
    }
    let Some(expires) = heavy_licence_expires_on(gross_kg, rate_change) else {
        return 0;
    };
    let days = days_between(purchase, expires).max(0) as u64;
    days.saturating_mul(u64::from(km_per_day))
}

/// Refund for the distance a licence did not use, pro rata to the licence distance.
///
/// Returns micro-dollars, rounding half up. The refund is the price paid,
/// scaled by the unused share of the licensed distance.
///
/// **Requires verification.** This is a model of a pro-rata refund, not an
/// NZTA rule. NZTA may refund differently, or not at all. Do not quote this
/// figure as a refund entitlement.
pub fn pro_rata_refund_micros(
    licence: &DistanceLicence,
    odometer_km: u64,
    paid_micros: u64,
) -> u64 {
    if licence.licensed_km == 0 {
        return 0;
    }
    let unused = licence.remaining_km(odometer_km).min(licence.licensed_km);
    let product = u128::from(paid_micros) * u128::from(unused);
    let licensed = u128::from(licence.licensed_km);
    // Half-up rounding: add half the divisor before dividing.
    let refund = (product + licensed / 2) / licensed;
    u64::try_from(refund).unwrap_or(u64::MAX)
}

/// Why exempt distance could not be deducted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExemptionError {
    /// The operator has no NZTA Section 43 approval for this recorder.
    NotApproved,
    /// The exempt distance is longer than the billable distance.
    ExemptExceedsBillable,
}

/// Subtracts off-road or other exempt distance from billable distance.
///
/// Claiming deductions requires an NZTA Approved Distance Recorder under
/// Section 43 of the Road User Charges Act. Without that approval this
/// function refuses, so an unapproved system cannot reduce a charge.
/// `section43_approved` must be `true` only when that approval is in force.
pub fn billable_after_exemption(
    billable_metres: u64,
    exempt_metres: u64,
    section43_approved: bool,
) -> Result<u64, ExemptionError> {
    if !section43_approved {
        return Err(ExemptionError::NotApproved);
    }
    billable_metres
        .checked_sub(exempt_metres)
        .ok_or(ExemptionError::ExemptExceedsBillable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: u16, month: u8, day: u8) -> RateDate {
        RateDate { year, month, day }
    }

    #[test]
    fn remaining_distance_counts_down_to_zero() {
        let licence = DistanceLicence {
            start_odometer_km: 10_000,
            licensed_km: 5_000,
        };
        assert_eq!(licence.expiry_odometer_km(), 15_000);
        assert_eq!(licence.remaining_km(12_000), 3_000);
        assert_eq!(licence.remaining_km(15_000), 0);
        assert_eq!(licence.remaining_km(99_999), 0);
    }

    #[test]
    fn a_licence_is_used_up_at_its_expiry_odometer() {
        let licence = DistanceLicence {
            start_odometer_km: 0,
            licensed_km: 100,
        };
        let today = date(2026, 10, 1);
        assert_eq!(
            status(&licence, 99, today, None),
            LicenceStatus::Valid { remaining_km: 1 }
        );
        assert_eq!(status(&licence, 100, today, None), LicenceStatus::UsedUp);
    }

    #[test]
    fn the_date_limit_expires_a_licence_with_distance_left() {
        let licence = DistanceLicence {
            start_odometer_km: 0,
            licensed_km: 100,
        };
        let limit = Some(date(2026, 9, 6));
        assert_eq!(
            status(&licence, 10, date(2026, 9, 6), limit),
            LicenceStatus::Valid { remaining_km: 90 }
        );
        assert_eq!(
            status(&licence, 10, date(2026, 9, 7), limit),
            LicenceStatus::ExpiredByDate
        );
    }

    #[test]
    fn used_up_takes_precedence_over_the_date_limit() {
        let licence = DistanceLicence {
            start_odometer_km: 0,
            licensed_km: 10,
        };
        assert_eq!(
            status(&licence, 10, date(2030, 1, 1), Some(date(2026, 1, 1))),
            LicenceStatus::UsedUp
        );
    }

    #[test]
    fn day_counts_respect_month_lengths_and_leap_years() {
        assert_eq!(days_between(date(2026, 8, 6), date(2026, 9, 6)), 31);
        assert_eq!(days_between(date(2024, 2, 28), date(2024, 3, 1)), 2);
        assert_eq!(days_between(date(2025, 2, 28), date(2025, 3, 1)), 1);
        assert_eq!(days_between(date(1970, 1, 1), date(1970, 1, 2)), 1);
        assert_eq!(days_between(date(2026, 9, 6), date(2026, 8, 6)), -31);
    }

    #[test]
    fn overlap_covers_the_days_from_purchase_to_expiry() {
        let rate_change = date(2026, 8, 6);
        // Bought 20 August, so the old heavy licence runs to 6 September: 17 days.
        let km = overlap_licence_km(44_000, date(2026, 8, 20), rate_change, 100);
        assert_eq!(km, 1_700);
    }

    #[test]
    fn no_overlap_for_light_vehicles_or_purchases_outside_the_window() {
        let rate_change = date(2026, 8, 6);
        assert_eq!(
            overlap_licence_km(3_000, date(2026, 8, 20), rate_change, 100),
            0
        );
        assert_eq!(
            overlap_licence_km(44_000, date(2026, 8, 1), rate_change, 100),
            0
        );
        assert_eq!(
            overlap_licence_km(44_000, date(2026, 9, 10), rate_change, 100),
            0
        );
    }

    #[test]
    fn refund_is_pro_rata_to_unused_distance() {
        let licence = DistanceLicence {
            start_odometer_km: 0,
            licensed_km: 1_000,
        };
        // 250 km driven, so 750 of 1,000 km unused. $100 paid refunds $75.
        assert_eq!(
            pro_rata_refund_micros(&licence, 250, 100_000_000),
            75_000_000
        );
        assert_eq!(pro_rata_refund_micros(&licence, 1_000, 100_000_000), 0);
    }

    #[test]
    fn refund_rounds_half_up() {
        let licence = DistanceLicence {
            start_odometer_km: 0,
            licensed_km: 3,
        };
        // One third of 1 micro-dollar rounds down to 0, two thirds rounds up to 1.
        assert_eq!(pro_rata_refund_micros(&licence, 2, 1), 0);
        assert_eq!(pro_rata_refund_micros(&licence, 1, 2), 1);
    }

    #[test]
    fn exempt_distance_is_refused_without_section_43_approval() {
        assert_eq!(
            billable_after_exemption(10_000, 2_000, false),
            Err(ExemptionError::NotApproved)
        );
    }

    #[test]
    fn exempt_distance_is_deducted_only_when_approved() {
        assert_eq!(billable_after_exemption(10_000, 2_000, true), Ok(8_000));
        assert_eq!(
            billable_after_exemption(1_000, 2_000, true),
            Err(ExemptionError::ExemptExceedsBillable)
        );
    }
}
