// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Fleet summary: charges and distance per vehicle and per month.
//!
//! Trips recorded with an inline vehicle, which has no registered vehicle,
//! appear with a null `vehicle_id` and are grouped by their vehicle type.

use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::Full;
use rusqlite::params;
use serde::Serialize;

use crate::error::ApiError;
use crate::reports::{date_range, km, nzd};
use crate::{json_response, AppState};

/// One line of the summary: a vehicle's totals for one month.
#[derive(Debug, Serialize)]
struct SummaryRow {
    month: String,
    vehicle_id: Option<String>,
    vehicle_name: Option<String>,
    rate_type: String,
    trips: i64,
    billed_km: String,
    charge_excl_gst_nzd: String,
    gst_nzd: String,
    charge_incl_gst_nzd: String,
}

/// `GET /v1/fleet/summary?from=&to=`: totals per vehicle and month.
pub async fn summary(
    state: &AppState,
    query: Option<&str>,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let range = date_range(query)?;
    let rows = state
        .db
        .run(move |connection| {
            let mut statement = connection.prepare(
                "SELECT substr(t.trip_date, 1, 7), t.vehicle_id, MAX(v.name), t.rate_type, \
                 COUNT(*), SUM(t.billed_metres), SUM(t.excl_gst_micros), SUM(t.gst_micros), \
                 SUM(t.incl_gst_micros) \
                 FROM trips t LEFT JOIN vehicles v ON v.id = t.vehicle_id \
                 WHERE t.trip_date BETWEEN ?1 AND ?2 \
                 GROUP BY 1, 2, 4 ORDER BY 1, 2, 4",
            )?;
            let rows = statement.query_map(params![range.from, range.to], |row| {
                Ok(SummaryRow {
                    month: row.get(0)?,
                    vehicle_id: row.get(1)?,
                    vehicle_name: row.get(2)?,
                    rate_type: row.get(3)?,
                    trips: row.get(4)?,
                    billed_km: km(row.get(5)?),
                    charge_excl_gst_nzd: nzd(row.get(6)?),
                    gst_nzd: nzd(row.get(7)?),
                    charge_incl_gst_nzd: nzd(row.get(8)?),
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(json_response(
        StatusCode::OK,
        &serde_json::json!({ "rows": rows }),
    ))
}
