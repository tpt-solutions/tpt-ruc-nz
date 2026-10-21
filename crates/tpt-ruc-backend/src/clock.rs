// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Calendar arithmetic on [`RateDate`], without a date-time dependency.
//!
//! The day-count conversions use the standard civil-calendar algorithms, which
//! are exact for the proleptic Gregorian calendar.

use std::time::{SystemTime, UNIX_EPOCH};

use tpt_ruc_core::ruc::RateDate;

/// Today's date in UTC.
pub fn today() -> RateDate {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    from_days(seconds.div_euclid(86_400))
}

/// Days since 1970-01-01. Negative before that date.
pub fn to_days(date: RateDate) -> i64 {
    let year = i64::from(date.year) - i64::from(date.month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(date.month);
    let day_of_year =
        (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(date.day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The date `days` days after 1970-01-01.
pub fn from_days(days: i64) -> RateDate {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    RateDate {
        year: year as u16,
        month: month as u8,
        day: day as u8,
    }
}

/// The date `days` days after `date`. Use a negative count to go back.
pub fn add_days(date: RateDate, days: i64) -> RateDate {
    from_days(to_days(date) + days)
}

/// Whole days from `from` to `to`. Negative when `to` is earlier.
pub fn days_between(from: RateDate, to: RateDate) -> i64 {
    to_days(to) - to_days(from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: u16, month: u8, day: u8) -> RateDate {
        RateDate { year, month, day }
    }

    #[test]
    fn the_epoch_is_day_zero() {
        assert_eq!(to_days(date(1970, 1, 1)), 0);
        assert_eq!(from_days(0), date(1970, 1, 1));
        assert_eq!(to_days(date(2000, 3, 1)), 11_017);
    }

    #[test]
    fn conversions_round_trip_across_leap_years() {
        for days in (-80_000..80_000).step_by(37) {
            assert_eq!(to_days(from_days(days)), days);
        }
        assert_eq!(from_days(to_days(date(2024, 2, 29))), date(2024, 2, 29));
    }

    #[test]
    fn adding_days_crosses_month_and_year_ends() {
        assert_eq!(add_days(date(2026, 12, 31), 1), date(2027, 1, 1));
        assert_eq!(add_days(date(2026, 3, 1), -1), date(2026, 2, 28));
        assert_eq!(add_days(date(2028, 2, 28), 1), date(2028, 2, 29));
    }

    #[test]
    fn days_between_is_signed() {
        assert_eq!(days_between(date(2026, 10, 1), date(2026, 10, 31)), 30);
        assert_eq!(days_between(date(2026, 10, 31), date(2026, 10, 1)), -30);
    }
}
