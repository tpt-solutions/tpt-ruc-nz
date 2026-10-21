// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Licence alerts for heavy vehicles.
//!
//! Only the rules `tpt-ruc-core` already encodes are used:
//!
//! - A heavy licence bought before a rate change expires one month after it
//!   (`heavy_licence_expires_on`).
//! - A heavy licence bought in the month after a rate change needs an overlap
//!   licence (`needs_overlap_licence`).
//!
//! The rate changes are the start dates of the rate tables the core knows. The
//! alert needs each vehicle's `licence_bought_on` date, which must be entered
//! on the vehicle. Vehicles without that date get no alerts.

use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::Full;
use serde::Serialize;
use tpt_ruc_core::ruc::{
    heavy_licence_expires_on, needs_overlap_licence, one_month_after, RateDate, HEAVY_OVER_KG,
};
use tpt_ruc_core::ruc_rates::NZTA_SCHEDULE;

use crate::clock::{days_between, today};
use crate::error::ApiError;
use crate::trips::{format_date, parse_date, query_number};
use crate::{json_response, AppState};

/// Default look-ahead window, in days.
pub const DEFAULT_WITHIN_DAYS: u32 = 30;
/// Longest look-ahead window accepted, in days.
pub const MAX_WITHIN_DAYS: u32 = 3_650;

/// A heavy vehicle's licence facts, as the alert calculation needs them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertVehicle {
    /// The vehicle's ID.
    pub id: String,
    /// The vehicle's name.
    pub name: String,
    /// RUC weight in kilograms.
    pub gross_kg: u32,
    /// When the current distance licence was bought.
    pub licence_bought_on: RateDate,
}

/// One alert for one vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Alert {
    /// The vehicle's ID.
    pub vehicle_id: String,
    /// The vehicle's name.
    pub vehicle_name: String,
    /// `expires_soon`, `expired` or `overlap_required`.
    pub kind: &'static str,
    /// The date the alert is about, as `YYYY-MM-DD`.
    pub date: String,
    /// Days from today until `date`. Negative once it has passed.
    pub days_remaining: i64,
}

/// Works out the alerts for `vehicles` as of `today`.
///
/// `rate_changes` must be in ascending order. Alerts for licence expiry are
/// included when they fall within `within_days` of today, or have already passed.
/// Overlap alerts are included while their window is still open.
pub fn alerts_at(
    vehicles: &[AlertVehicle],
    today: RateDate,
    within_days: i64,
    rate_changes: &[RateDate],
) -> Vec<Alert> {
    let mut alerts = Vec::new();
    for vehicle in vehicles.iter().filter(|v| v.gross_kg > HEAVY_OVER_KG) {
        let purchase = vehicle.licence_bought_on;

        if let Some(expiry) = rate_changes
            .iter()
            .find(|&&change| purchase < change)
            .and_then(|&change| heavy_licence_expires_on(vehicle.gross_kg, change))
        {
            let days = days_between(today, expiry);
            if days <= within_days {
                alerts.push(alert(
                    vehicle,
                    if days < 0 { "expired" } else { "expires_soon" },
                    expiry,
                    days,
                ));
            }
        }

        for &change in rate_changes {
            if needs_overlap_licence(vehicle.gross_kg, purchase, change) {
                let window_end = one_month_after(change);
                let days = days_between(today, window_end);
                if days >= 0 && days <= within_days {
                    alerts.push(alert(vehicle, "overlap_required", window_end, days));
                }
            }
        }
    }
    alerts.sort_by_key(|a| a.days_remaining);
    alerts
}

fn alert(vehicle: &AlertVehicle, kind: &'static str, date: RateDate, days: i64) -> Alert {
    Alert {
        vehicle_id: vehicle.id.clone(),
        vehicle_name: vehicle.name.clone(),
        kind,
        date: format_date(date),
        days_remaining: days,
    }
}

/// `GET /v1/alerts?within_days=N`: licence alerts for heavy vehicles.
pub async fn list(
    state: &AppState,
    query: Option<&str>,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let within_days = query_number(query, "within_days")?
        .unwrap_or(DEFAULT_WITHIN_DAYS)
        .min(MAX_WITHIN_DAYS);
    let now = today();
    let vehicles = state
        .db
        .run(|connection| {
            let mut statement = connection.prepare(
                "SELECT id, name, gross_kg, licence_bought_on FROM vehicles \
                 WHERE licence_bought_on IS NOT NULL",
            )?;
            let rows = statement.query_map([], |row| {
                let gross: i64 = row.get(2)?;
                let bought: String = row.get(3)?;
                Ok((row.get(0)?, row.get(1)?, gross, bought))
            })?;
            let mut vehicles = Vec::new();
            for row in rows {
                let (id, name, gross, bought) = row?;
                // Stored dates were checked when the vehicle was saved, so a parse
                // failure here means the database was changed by hand. Skip it.
                let Some(licence_bought_on) = parse_date(&bought) else {
                    continue;
                };
                vehicles.push(AlertVehicle {
                    id,
                    name,
                    gross_kg: u32::try_from(gross).unwrap_or(0),
                    licence_bought_on,
                });
            }
            Ok(vehicles)
        })
        .await?;

    let changes = rate_changes();
    let alerts = alerts_at(&vehicles, now, i64::from(within_days), &changes);
    Ok(json_response(
        StatusCode::OK,
        &serde_json::json!({
            "today": format_date(now),
            "within_days": within_days,
            "alerts": alerts,
        }),
    ))
}

/// The rate-change dates the core knows about, in ascending order.
pub fn rate_changes() -> Vec<RateDate> {
    let mut changes: Vec<RateDate> = NZTA_SCHEDULE.iter().map(|table| table.effective).collect();
    changes.sort();
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: u16, month: u8, day: u8) -> RateDate {
        RateDate { year, month, day }
    }

    fn truck(gross_kg: u32, bought: RateDate) -> AlertVehicle {
        AlertVehicle {
            id: "v_1".to_owned(),
            name: "Truck".to_owned(),
            gross_kg,
            licence_bought_on: bought,
        }
    }

    const CHANGE: fn() -> Vec<RateDate> = || vec![date(2026, 8, 6)];

    #[test]
    fn a_heavy_licence_bought_before_a_change_expires_a_month_after_it() {
        let vehicles = [truck(20_000, date(2026, 7, 1))];
        // Expiry is 2026-09-06. Thirty days before it, the alert is due.
        let alerts = alerts_at(&vehicles, date(2026, 8, 7), 30, &CHANGE());
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, "expires_soon");
        assert_eq!(alerts[0].date, "2026-09-06");
        assert_eq!(alerts[0].days_remaining, 30);
    }

    #[test]
    fn outside_the_window_there_is_no_alert() {
        let vehicles = [truck(20_000, date(2026, 7, 1))];
        assert!(alerts_at(&vehicles, date(2026, 7, 1), 30, &CHANGE()).is_empty());
    }

    #[test]
    fn a_passed_expiry_is_reported_as_expired() {
        let vehicles = [truck(20_000, date(2026, 7, 1))];
        let alerts = alerts_at(&vehicles, date(2026, 10, 1), 30, &CHANGE());
        assert_eq!(alerts[0].kind, "expired");
        assert!(alerts[0].days_remaining < 0);
    }

    #[test]
    fn light_vehicles_and_licences_bought_after_the_last_change_get_no_expiry_alert() {
        let light = [truck(3_000, date(2026, 7, 1))];
        assert!(alerts_at(&light, date(2026, 9, 1), 3_650, &CHANGE()).is_empty());
        // Bought after the overlap month, so there is no expiry to warn about.
        let after = [truck(20_000, date(2026, 9, 20))];
        assert!(alerts_at(&after, date(2026, 9, 1), 3_650, &CHANGE()).is_empty());
    }

    #[test]
    fn buying_inside_the_month_after_a_change_needs_an_overlap_licence() {
        let vehicles = [truck(20_000, date(2026, 8, 20))];
        let alerts = alerts_at(&vehicles, date(2026, 8, 25), 30, &CHANGE());
        let overlap: Vec<_> = alerts
            .iter()
            .filter(|a| a.kind == "overlap_required")
            .collect();
        assert_eq!(overlap.len(), 1);
        assert_eq!(overlap[0].date, "2026-09-06");
    }

    #[test]
    fn the_change_list_is_sorted_and_taken_from_the_core() {
        let changes = rate_changes();
        assert!(changes.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(changes.first(), Some(&date(2026, 8, 6)));
    }
}
