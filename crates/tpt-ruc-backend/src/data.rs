// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Export and deletion of data.
//!
//! The export is the set of stored trips the caller may see, as JSON, so a
//! person can move or keep a copy of their own data. Deletion removes trips by
//! an inclusive date range. Both `from` and `to` are required, so nothing is
//! deleted by accident. To delete everything, pass the widest range.
//!
//! Drivers export and delete only their own trips. Managers and admins work
//! on every trip.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::Full;
use rusqlite::{params, Connection};
use serde_json::json;

use crate::auth::Principal;
use crate::clock::{add_days, today};
use crate::error::ApiError;
use crate::trips::{format_date, parse_date, trip_row, TRIP_COLUMNS};
use crate::{json_response, query_value, AppState};

/// Identifies the export file, so a future reader can check what it is.
pub const EXPORT_FORMAT: &str = "tpt-ruc-export";
/// Version of the export layout. Bump it when a field changes meaning.
pub const EXPORT_VERSION: u32 = 1;

/// `GET /v1/export`: the trips the caller may see, oldest first.
pub async fn export(
    state: &AppState,
    principal: &Principal,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let scope = principal.trip_scope().map(str::to_owned);
    let trips = state
        .db
        .run(move |connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {TRIP_COLUMNS} FROM trips WHERE (?1 IS NULL OR user_id = ?1) \
                 ORDER BY trip_date, id"
            ))?;
            let rows = statement.query_map(params![scope], trip_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;

    Ok(json_response(
        StatusCode::OK,
        &json!({ "format": EXPORT_FORMAT, "version": EXPORT_VERSION, "trips": trips }),
    ))
}

/// `DELETE /v1/trips?from=YYYY-MM-DD&to=YYYY-MM-DD`: deletes the caller's trips in the inclusive range.
pub async fn delete_trips(
    state: &AppState,
    principal: &Principal,
    query: Option<&str>,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let from = required_date(query, "from")?;
    let to = required_date(query, "to")?;
    if from > to {
        return Err(ApiError::BadRequest("from must not be after to".to_owned()));
    }

    let scope = principal.trip_scope().map(str::to_owned);
    let deleted = {
        let (from, to) = (from.clone(), to.clone());
        state
            .db
            .run(move |connection| {
                Ok(connection.execute(
                    "DELETE FROM trips WHERE trip_date BETWEEN ?1 AND ?2 \
                     AND (?3 IS NULL OR user_id = ?3)",
                    params![from, to, scope],
                )?)
            })
            .await?
    };
    tracing::info!(deleted, %from, %to, "trips deleted");
    Ok(json_response(
        StatusCode::OK,
        &json!({ "deleted": deleted }),
    ))
}

/// Reads a required `YYYY-MM-DD` query parameter.
fn required_date(query: Option<&str>, name: &str) -> Result<String, ApiError> {
    let value = query_value(query, name)
        .ok_or_else(|| ApiError::BadRequest(format!("{name} is required")))?;
    parse_date(value).ok_or_else(|| ApiError::BadRequest(format!("{name} must be YYYY-MM-DD")))?;
    Ok(value.to_owned())
}

/// Deletes every trip dated before `cutoff` (`YYYY-MM-DD`). Returns how many were deleted.
pub fn prune_before(connection: &Connection, cutoff: &str) -> rusqlite::Result<usize> {
    connection.execute("DELETE FROM trips WHERE trip_date < ?1", params![cutoff])
}

/// The first date kept when trips are kept for `days` days, counting from today.
pub fn retention_cutoff(days: u32) -> String {
    format_date(add_days(today(), -i64::from(days)))
}

/// Runs retention in the background: once at start-up and then every `every`.
/// Each run deletes trips older than `days` and logs how many were removed.
pub async fn run_retention(state: Arc<AppState>, days: u32, every: Duration) {
    let mut ticker = tokio::time::interval(every);
    loop {
        ticker.tick().await;
        let cutoff = retention_cutoff(days);
        let result = state
            .db
            .run(move |connection| Ok(prune_before(connection, &cutoff)?))
            .await;
        match result {
            Ok(0) => {}
            Ok(removed) => tracing::info!(removed, days, "retention removed old trips"),
            Err(_) => tracing::error!("retention run failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pruning_removes_only_trips_before_the_cutoff() {
        let connection = Connection::open_in_memory().unwrap();
        crate::db::migrate(&connection).unwrap();
        for (id, date) in [
            ("old", "2026-01-01"),
            ("edge", "2026-06-30"),
            ("new", "2026-07-01"),
        ] {
            connection
                .execute(
                    "INSERT INTO trips (owner_key, id, trip_date, rate_type, gross_kg, electric, metres,                      rate_version, billed_metres, incl_gst_micros, excl_gst_micros, gst_micros)                      VALUES ('owner', ?1, ?2, '1', 3000, 0, 1.0, 'v', 0, 0, 0, 0)",
                    params![id, date],
                )
                .unwrap();
        }

        assert_eq!(prune_before(&connection, "2026-07-01").unwrap(), 2);
        let left: i64 = connection
            .query_row("SELECT COUNT(*) FROM trips", [], |row| row.get(0))
            .unwrap();
        assert_eq!(left, 1);
    }

    #[test]
    fn the_cutoff_is_the_retention_period_before_today() {
        let cutoff = retention_cutoff(0);
        assert_eq!(cutoff, format_date(today()));
        assert!(retention_cutoff(365) < cutoff);
    }

    #[test]
    fn dates_are_required_and_validated() {
        assert!(required_date(None, "from").is_err());
        assert!(required_date(Some("to=2026-10-01"), "from").is_err());
        assert!(required_date(Some("from=2026-02-30"), "from").is_err());
        assert_eq!(
            required_date(Some("from=2026-10-01"), "from").unwrap(),
            "2026-10-01"
        );
    }
}
