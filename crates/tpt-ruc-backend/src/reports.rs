// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! CSV reports for the owner's records and for NZTA audits.
//!
//! Both reports take an optional inclusive date range, `from` and `to`, as
//! `YYYY-MM-DD`. Lines end in CRLF, as RFC 4180 specifies.
//!
//! Money is shown in NZD dollars to two decimal places. Each total is rounded
//! half up once, from the exact micro-dollar sum, so a report's totals match
//! the sum of its own unrounded amounts.

use bytes::Bytes;
use http::{header, HeaderValue, Response, StatusCode};
use http_body_util::Full;
use rusqlite::params;

use crate::error::ApiError;
use crate::trips::{parse_date, trip_row, TRIP_COLUMNS};
use crate::{query_value, AppState};

/// Start of the open range, used when `from` is absent.
const FIRST_DATE: &str = "0000-01-01";
/// End of the open range, used when `to` is absent.
const LAST_DATE: &str = "9999-12-31";

const TRIP_HEADER: &str = "trip_id,trip_date,vehicle_type,gross_kg,electric,distance_m,billed_km,\
    rate_version,charge_excl_gst_nzd,gst_nzd,charge_incl_gst_nzd";
const MONTHLY_HEADER: &str = "month,vehicle_type,trips,billed_km,charge_excl_gst_nzd,gst_nzd,\
    charge_incl_gst_nzd";

/// The inclusive date range a report covers. Both ends are `YYYY-MM-DD`, so
/// string order is date order.
pub(crate) struct DateRange {
    pub(crate) from: String,
    pub(crate) to: String,
}

/// Reads `from` and `to` from the query string, applying open ends where absent.
pub(crate) fn date_range(query: Option<&str>) -> Result<DateRange, ApiError> {
    let from = bound(query, "from", FIRST_DATE)?;
    let to = bound(query, "to", LAST_DATE)?;
    if from > to {
        return Err(ApiError::BadRequest("from must not be after to".to_owned()));
    }
    Ok(DateRange { from, to })
}

fn bound(query: Option<&str>, name: &str, default: &str) -> Result<String, ApiError> {
    match query_value(query, name) {
        Some(value) => {
            parse_date(value)
                .ok_or_else(|| ApiError::BadRequest(format!("{name} must be YYYY-MM-DD")))?;
            Ok(value.to_owned())
        }
        None => Ok(default.to_owned()),
    }
}

/// `GET /v1/reports/trips.csv`: one line per stored trip, for audit.
pub async fn trips_csv(
    state: &AppState,
    query: Option<&str>,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let range = date_range(query)?;
    let rows = state
        .db
        .run(move |connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {TRIP_COLUMNS} FROM trips WHERE trip_date BETWEEN ?1 AND ?2 \
                 ORDER BY trip_date, id"
            ))?;
            let rows = statement.query_map(params![range.from, range.to], trip_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;

    let mut csv = header_line(TRIP_HEADER);
    for row in &rows {
        csv.push_str(&data_line(&[
            csv_field(&row.id),
            csv_field(&row.trip_date),
            csv_field(&row.rate_type),
            row.gross_kg.to_string(),
            row.electric.to_string(),
            format!("{:.3}", row.metres),
            km(row.billed_metres),
            csv_field(&row.rate_version),
            nzd(row.excl_gst_micros),
            nzd(row.gst_micros),
            nzd(row.incl_gst_micros),
        ]));
    }
    Ok(csv_response("trips.csv", csv))
}

/// One vehicle type's totals for one month.
struct MonthlyRow {
    month: String,
    vehicle_type: String,
    trips: i64,
    billed_metres: i64,
    excl_gst_micros: i64,
    gst_micros: i64,
    incl_gst_micros: i64,
}

/// Loads the monthly totals for `range`, one row per month and vehicle type.
async fn monthly_rows(state: &AppState, range: DateRange) -> Result<Vec<MonthlyRow>, ApiError> {
    let rows = state
        .db
        .run(move |connection| {
            let mut statement = connection.prepare(
                "SELECT substr(trip_date, 1, 7), rate_type, COUNT(*), SUM(billed_metres), \
                 SUM(excl_gst_micros), SUM(gst_micros), SUM(incl_gst_micros) \
                 FROM trips WHERE trip_date BETWEEN ?1 AND ?2 \
                 GROUP BY 1, 2 ORDER BY 1, 2",
            )?;
            let rows = statement.query_map(params![range.from, range.to], |row| {
                Ok(MonthlyRow {
                    month: row.get(0)?,
                    vehicle_type: row.get(1)?,
                    trips: row.get(2)?,
                    billed_metres: row.get(3)?,
                    excl_gst_micros: row.get(4)?,
                    gst_micros: row.get(5)?,
                    incl_gst_micros: row.get(6)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(rows)
}

/// `GET /v1/reports/monthly.csv`: totals per calendar month and vehicle type.
pub async fn monthly_csv(
    state: &AppState,
    query: Option<&str>,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let range = date_range(query)?;
    let rows = monthly_rows(state, range).await?;

    let mut csv = header_line(MONTHLY_HEADER);
    for row in &rows {
        csv.push_str(&data_line(&[
            csv_field(&row.month),
            csv_field(&row.vehicle_type),
            row.trips.to_string(),
            km(row.billed_metres),
            nzd(row.excl_gst_micros),
            nzd(row.gst_micros),
            nzd(row.incl_gst_micros),
        ]));
    }
    Ok(csv_response("monthly.csv", csv))
}

/// `GET /v1/reports/monthly.pdf`: the monthly totals as a PDF table.
pub async fn monthly_pdf(
    state: &AppState,
    query: Option<&str>,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let range = date_range(query)?;
    let subtitle = format!("Trips from {} to {}", range.from, range.to);
    let rows = monthly_rows(state, range).await?;

    let header = format!(
        "{:<9} {:<8} {:>6} {:>10} {:>12} {:>10} {:>12}",
        "month", "vehicle", "trips", "billed km", "excl GST", "GST", "incl GST"
    );
    let lines: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "{:<9} {:<8} {:>6} {:>10} {:>12} {:>10} {:>12}",
                row.month,
                row.vehicle_type,
                row.trips,
                km(row.billed_metres),
                format!("${}", nzd(row.excl_gst_micros)),
                format!("${}", nzd(row.gst_micros)),
                format!("${}", nzd(row.incl_gst_micros)),
            )
        })
        .collect();

    let bytes = crate::pdf::table_document("Monthly RUC totals", &subtitle, &header, &lines)
        .map_err(|error| {
            tracing::error!(%error, "could not build the PDF report");
            ApiError::Internal
        })?;
    let mut response = Response::new(Full::new(Bytes::from(bytes)));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"monthly.pdf\""),
    );
    Ok(response)
}

/// A header line, terminated with CRLF.
pub(crate) fn header_line(header: &str) -> String {
    format!("{header}\r\n")
}

/// Joins fields into one CRLF-terminated line. The fields must already be escaped.
pub(crate) fn data_line(fields: &[String]) -> String {
    format!("{}\r\n", fields.join(","))
}

/// Escapes one text field for CSV.
///
/// Fields that contain a comma, quote or line break are quoted. A field that
/// starts with a character spreadsheets read as a formula is prefixed with an
/// apostrophe, so a trip ID cannot run a formula when the report is opened.
pub fn csv_field(value: &str) -> String {
    let neutral = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if neutral.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", neutral.replace('"', "\"\""))
    } else {
        neutral
    }
}

/// Formats micro-dollars as NZD with two decimal places, rounding half up.
pub fn nzd(micros: i64) -> String {
    let sign = if micros < 0 { "-" } else { "" };
    let cents = (micros.unsigned_abs() + 5_000) / 10_000;
    format!("{sign}{}.{:02}", cents / 100, cents % 100)
}

/// Formats whole metres as kilometres with three decimal places.
pub fn km(metres: i64) -> String {
    format!("{}.{:03}", metres / 1_000, metres.rem_euclid(1_000))
}

/// Builds a CSV download response.
pub(crate) fn csv_response(filename: &str, body: String) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_end_in_crlf() {
        assert_eq!(header_line("a,b"), "a,b\r\n");
        assert_eq!(data_line(&["x".to_owned(), "y".to_owned()]), "x,y\r\n");
    }

    #[test]
    fn plain_text_is_left_alone() {
        assert_eq!(csv_field("trip-1"), "trip-1");
    }

    #[test]
    fn fields_with_separators_are_quoted_and_quotes_doubled() {
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn formula_like_fields_are_neutralised() {
        assert_eq!(csv_field("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(csv_field("+1"), "'+1");
        assert_eq!(csv_field("-2"), "'-2");
        assert_eq!(csv_field("@SUM(A1)"), "'@SUM(A1)");
    }

    #[test]
    fn money_rounds_half_up_to_cents() {
        assert_eq!(nzd(0), "0.00");
        assert_eq!(nzd(938_220), "0.94");
        assert_eq!(nzd(1_234_567), "1.23");
        assert_eq!(nzd(1_235_000), "1.24");
        assert_eq!(nzd(4_999), "0.00");
        assert_eq!(nzd(5_000), "0.01");
        assert_eq!(nzd(-1_500_000), "-1.50");
    }

    #[test]
    fn kilometres_show_three_decimals() {
        assert_eq!(km(0), "0.000");
        assert_eq!(km(12_345), "12.345");
        assert_eq!(km(7), "0.007");
    }

    #[test]
    fn date_range_defaults_to_open_and_rejects_bad_bounds() {
        let open = date_range(None).unwrap();
        assert_eq!(
            (open.from.as_str(), open.to.as_str()),
            (FIRST_DATE, LAST_DATE)
        );
        assert!(date_range(Some("from=2026-02-30")).is_err());
        assert!(date_range(Some("from=2026-10-02&to=2026-10-01")).is_err());
        assert!(date_range(Some("from=2026-10-01&to=2026-10-31")).is_ok());
    }
}
