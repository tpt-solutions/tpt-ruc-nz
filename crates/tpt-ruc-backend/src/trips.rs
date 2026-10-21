// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Trip ingest and listing.
//!
//! Clients send trip summaries: distance and vehicle, never raw GPS points. The
//! server recomputes every charge with `tpt-ruc-core` and ignores any amount the
//! client might send, so stored liability is always the server's own figure.
//!
//! A trip names its vehicle either with `vehicle_id` (a registered vehicle) or
//! with an inline `vehicle` object. Drivers may only use registered vehicles
//! that are assigned to them.

use bytes::Bytes;
use http::{HeaderMap, Response, StatusCode};
use http_body_util::Full;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tpt_ruc_core::ruc::{liability, RateDate, Vehicle};
use tpt_ruc_core::ruc_rates::NZTA_SCHEDULE;
use tpt_ruc_core::{Liability, RucError};

use crate::auth::{Principal, Role};
use crate::devices;
use crate::error::ApiError;
use crate::vehicles::visible_vehicle;
use crate::{json_response, query_value, AppState};

/// Most trips accepted in one request.
pub const MAX_BATCH: usize = 500;
/// Longest trip ID accepted, in bytes.
pub const MAX_ID_LEN: usize = 64;
const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 1_000;

/// One trip summary as the client sends it.
#[derive(Debug, Deserialize)]
pub struct TripInput {
    /// Client-chosen unique ID. Resending the same ID with the same values is safe.
    pub id: String,
    /// Trip date as `YYYY-MM-DD`.
    pub date: String,
    /// Distance driven, in metres.
    pub metres: f64,
    /// A registered vehicle, by ID. Use this or `vehicle`, not both.
    #[serde(default)]
    pub vehicle_id: Option<String>,
    /// The vehicle, given inline. Not allowed for drivers.
    #[serde(default)]
    pub vehicle: Option<VehicleInput>,
}

/// The vehicle fields that decide the charge, given inline.
#[derive(Debug, Deserialize)]
pub struct VehicleInput {
    /// NZTA RUC vehicle type code, for example `"1"` or `"H61"`.
    pub rate_type: String,
    /// RUC weight in kilograms.
    pub gross_kg: u32,
    /// True for a battery electric vehicle.
    #[serde(default)]
    pub electric: bool,
}

/// Body of `POST /v1/trips`.
#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    /// The trips to store. The whole batch is stored, or none of it is.
    pub trips: Vec<TripInput>,
}

/// What happened to one trip.
#[derive(Debug, Serialize)]
pub struct ItemResult {
    /// The trip ID from the request.
    pub id: String,
    /// `created`, `unchanged`, `rejected`, `conflict` or `not_stored`.
    pub status: &'static str,
    /// Why the trip was rejected or conflicts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The server's charge for the trip, when it was calculated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liability: Option<Liability>,
}

/// A trip that passed validation, with the server's charge.
#[derive(Debug)]
struct ValidTrip {
    id: String,
    date: String,
    rate_type: &'static str,
    gross_kg: u32,
    electric: bool,
    metres: f64,
    vehicle_id: Option<String>,
    liability: Liability,
}

impl ValidTrip {
    /// True when `stored` holds the same client-supplied values, and belongs to the same owner.
    fn matches(&self, stored: &StoredTrip, owner: Option<&str>) -> bool {
        stored.user_id.as_deref() == owner
            && stored.vehicle_id.as_deref() == self.vehicle_id.as_deref()
            && stored.trip_date == self.date
            && stored.rate_type == self.rate_type
            && stored.gross_kg == i64::from(self.gross_kg)
            && stored.electric == self.electric
            && stored.metres == self.metres
    }
}

/// The client-supplied columns of a stored trip, used to detect conflicts.
struct StoredTrip {
    trip_date: String,
    rate_type: String,
    gross_kg: i64,
    electric: bool,
    metres: f64,
    user_id: Option<String>,
    vehicle_id: Option<String>,
}

/// One stored trip, as the list and export endpoints return it.
#[derive(Debug, Serialize)]
pub struct TripRow {
    pub(crate) id: String,
    pub(crate) trip_date: String,
    pub(crate) rate_type: String,
    pub(crate) gross_kg: i64,
    pub(crate) electric: bool,
    pub(crate) metres: f64,
    pub(crate) rate_version: String,
    pub(crate) billed_metres: i64,
    pub(crate) incl_gst_micros: i64,
    pub(crate) excl_gst_micros: i64,
    pub(crate) gst_micros: i64,
    /// The user who recorded the trip, or `None` for trips recorded with the owner key.
    pub(crate) user_id: Option<String>,
    /// The registered vehicle, or `None` for a trip with an inline vehicle.
    pub(crate) vehicle_id: Option<String>,
}

/// `POST /v1/trips`: validates a batch, then stores it atomically.
///
/// A trip ID that is already stored with the same values is reported as
/// `unchanged`, so a retried upload is harmless. The same ID with different
/// values is a conflict, and the whole batch is refused. The trips are recorded
/// against the caller, whatever the body says.
pub async fn ingest(
    state: &AppState,
    principal: &Principal,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    // Devices sign the trips in this canonical form: compact JSON with keys in sorted order.
    let trips_json = serde_json::to_string(value.get("trips").unwrap_or(&serde_json::Value::Null))
        .map_err(|_| ApiError::Internal)?;
    let upload = devices::upload_from_headers(headers)?;
    let request: IngestRequest = serde_json::from_value(value)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    if request.trips.is_empty() {
        return Err(ApiError::BadRequest("trips must not be empty".to_owned()));
    }
    if request.trips.len() > MAX_BATCH {
        return Err(ApiError::BadRequest(format!(
            "at most {MAX_BATCH} trips per request"
        )));
    }

    let owner = principal.user_id().map(str::to_owned);
    let is_driver = principal.role() == Role::Driver;
    let driver = is_driver.then(|| owner.clone()).flatten();
    let results = state
        .db
        .run(move |connection| {
            // A device upload is recorded against the device's user, and the signature and
            // sequence number are checked before anything is stored.
            let device = match &upload {
                Some(upload) => Some(devices::verify_upload(connection, upload, &trips_json)?),
                None => None,
            };
            let owner = match &device {
                Some(device) => {
                    if is_driver && driver.as_deref() != Some(device.user_id.as_str()) {
                        return Err(ApiError::Forbidden);
                    }
                    Some(device.user_id.clone())
                }
                None => owner,
            };
            let device_sequence = upload
                .as_ref()
                .map(|upload| (upload.device_id.as_str(), upload.sequence));

            let mut valid = Vec::with_capacity(request.trips.len());
            let mut results = Vec::with_capacity(request.trips.len());
            let mut any_rejected = false;
            for input in &request.trips {
                match resolve(connection, input, driver.as_deref())? {
                    Ok(trip) => {
                        valid.push(trip);
                        results.push(ItemResult {
                            id: input.id.clone(),
                            status: "not_stored",
                            error: None,
                            liability: None,
                        });
                    }
                    Err(message) => {
                        any_rejected = true;
                        results.push(ItemResult {
                            id: input.id.clone(),
                            status: "rejected",
                            error: Some(message),
                            liability: None,
                        });
                    }
                }
            }
            if any_rejected {
                return Err(ApiError::Validation(results));
            }
            store_batch(
                connection,
                &valid,
                results,
                owner.as_deref(),
                device_sequence,
            )
        })
        .await?;
    Ok(json_response(
        StatusCode::OK,
        &json!({ "results": results }),
    ))
}

/// The columns of [`TripRow`], in the order [`trip_row`] reads them.
pub(crate) const TRIP_COLUMNS: &str = "id, trip_date, rate_type, gross_kg, electric, metres, \
    rate_version, billed_metres, incl_gst_micros, excl_gst_micros, gst_micros, user_id, vehicle_id";

/// Reads one stored trip from a row selected with [`TRIP_COLUMNS`].
pub(crate) fn trip_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TripRow> {
    Ok(TripRow {
        id: row.get(0)?,
        trip_date: row.get(1)?,
        rate_type: row.get(2)?,
        gross_kg: row.get(3)?,
        electric: row.get(4)?,
        metres: row.get(5)?,
        rate_version: row.get(6)?,
        billed_metres: row.get(7)?,
        incl_gst_micros: row.get(8)?,
        excl_gst_micros: row.get(9)?,
        gst_micros: row.get(10)?,
        user_id: row.get(11)?,
        vehicle_id: row.get(12)?,
    })
}

/// `GET /v1/trips`: lists stored trips, oldest first.
pub async fn list(
    state: &AppState,
    principal: &Principal,
    query: Option<&str>,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let limit = query_number(query, "limit")?
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let offset = query_number(query, "offset")?.unwrap_or(0);
    let scope = principal.trip_scope().map(str::to_owned);

    let trips = state
        .db
        .run(move |connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {TRIP_COLUMNS} FROM trips WHERE (?3 IS NULL OR user_id = ?3) \
                 ORDER BY trip_date, id LIMIT ?1 OFFSET ?2"
            ))?;
            let rows = statement.query_map(
                params![i64::from(limit), i64::from(offset), scope],
                trip_row,
            )?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;

    Ok(json_response(
        StatusCode::OK,
        &json!({ "trips": trips, "limit": limit, "offset": offset }),
    ))
}

/// Works out one trip's vehicle and charge, reading the database where needed.
///
/// The outer `Result` carries database failures. The inner one carries the
/// reason the trip is rejected, which is shown to the client.
fn resolve(
    connection: &Connection,
    input: &TripInput,
    driver: Option<&str>,
) -> Result<Result<ValidTrip, String>, ApiError> {
    let (date, metres) = match check_common(input) {
        Ok(common) => common,
        Err(message) => return Ok(Err(message)),
    };
    let trip = match (&input.vehicle, &input.vehicle_id) {
        (Some(_), Some(_)) => Err("give either vehicle or vehicle_id, not both".to_owned()),
        (None, None) => Err("give either vehicle or vehicle_id".to_owned()),
        (Some(_), None) if driver.is_some() => {
            Err("drivers must record trips against a vehicle assigned to them".to_owned())
        }
        (Some(inline), None) => charge(
            input,
            date,
            metres,
            &inline.rate_type,
            inline.gross_kg,
            inline.electric,
            None,
        ),
        (None, Some(vehicle_id)) => match visible_vehicle(connection, vehicle_id, driver)? {
            Some(vehicle) => charge(
                input,
                date,
                metres,
                &vehicle.rate_type,
                vehicle.gross_kg,
                vehicle.electric,
                Some(vehicle_id.clone()),
            ),
            None => Err("vehicle not found or not assigned to you".to_owned()),
        },
    };
    Ok(trip)
}

/// Checks the trip's ID, date and distance, which every trip must have.
fn check_common(input: &TripInput) -> Result<(RateDate, f64), String> {
    let id = input.id.trim();
    if id.is_empty() || input.id.len() > MAX_ID_LEN {
        return Err(format!("id must be 1 to {MAX_ID_LEN} characters"));
    }
    let date = parse_date(&input.date).ok_or_else(|| "date must be YYYY-MM-DD".to_owned())?;
    if !input.metres.is_finite() || input.metres < 0.0 {
        return Err("metres must be a non-negative number".to_owned());
    }
    Ok((date, input.metres))
}

/// Calculates the charge for a vehicle's rate fields. The error is shown to the client.
fn charge(
    input: &TripInput,
    date: RateDate,
    metres: f64,
    rate_type: &str,
    gross_kg: u32,
    electric: bool,
    vehicle_id: Option<String>,
) -> Result<ValidTrip, String> {
    if gross_kg == 0 {
        return Err("gross_kg must be above zero".to_owned());
    }
    let rate_type = known_type_code(rate_type)
        .ok_or_else(|| format!("unknown vehicle type code {rate_type}"))?;
    let vehicle = Vehicle {
        rate_type,
        gross_kg,
        electric,
    };
    let liability = liability(NZTA_SCHEDULE, &vehicle, date, metres)
        .map_err(|error| describe_rate_error(error, date))?;
    Ok(ValidTrip {
        id: input.id.trim().to_owned(),
        date: format_date(date),
        rate_type,
        gross_kg,
        electric,
        metres,
        vehicle_id,
        liability,
    })
}

/// Stores a validated batch in one transaction. Any conflict rolls the whole batch back.
fn store_batch(
    connection: &mut Connection,
    valid: &[ValidTrip],
    mut results: Vec<ItemResult>,
    owner: Option<&str>,
    device: Option<(&str, u64)>,
) -> Result<Vec<ItemResult>, ApiError> {
    let transaction = connection.transaction()?;
    if let Some((device_id, sequence)) = device {
        // Accept the upload only if its sequence number is newer than any accepted so far.
        // This runs in the same transaction as the inserts, so a failure stores nothing.
        let accepted = transaction.execute(
            "UPDATE devices SET last_sequence = ?2 \n             WHERE id = ?1 AND revoked = 0 AND last_sequence < ?2",
            params![device_id, sequence as i64],
        )?;
        if accepted == 0 {
            return Err(ApiError::Replay);
        }
    }
    let mut conflicts = false;
    for (trip, result) in valid.iter().zip(results.iter_mut()) {
        let stored = transaction
            .query_row(
                "SELECT trip_date, rate_type, gross_kg, electric, metres, user_id, vehicle_id \
                 FROM trips WHERE owner_key = ?2 AND id = ?1",
                params![trip.id, owner_key(owner)],
                |row| {
                    Ok(StoredTrip {
                        trip_date: row.get(0)?,
                        rate_type: row.get(1)?,
                        gross_kg: row.get(2)?,
                        electric: row.get(3)?,
                        metres: row.get(4)?,
                        user_id: row.get(5)?,
                        vehicle_id: row.get(6)?,
                    })
                },
            )
            .optional()?;

        match stored {
            Some(stored) if trip.matches(&stored, owner) => result.status = "unchanged",
            Some(_) => {
                conflicts = true;
                result.status = "conflict";
                result.error = Some("this ID is already stored with different values".to_owned());
            }
            None => {
                insert(&transaction, trip, owner)?;
                result.status = "created";
            }
        }
        result.liability = Some(trip.liability);
    }

    if conflicts {
        // Returning early drops the transaction, which rolls it back.
        for result in results.iter_mut().filter(|r| r.status != "conflict") {
            result.status = "not_stored";
            result.liability = None;
        }
        return Err(ApiError::Conflict(results));
    }

    transaction.commit()?;
    Ok(results)
}

/// Stores one validated trip, with the rate version that produced its charge.
fn insert(
    transaction: &rusqlite::Transaction<'_>,
    trip: &ValidTrip,
    owner: Option<&str>,
) -> Result<(), ApiError> {
    transaction.execute(
        "INSERT INTO trips (owner_key, id, trip_date, rate_type, gross_kg, electric, metres, \
         rate_version, billed_metres, incl_gst_micros, excl_gst_micros, gst_micros, user_id, \
         vehicle_id) \
         VALUES (?14, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            trip.id,
            trip.date,
            trip.rate_type,
            i64::from(trip.gross_kg),
            trip.electric,
            trip.metres,
            trip.liability.rate_version,
            to_db(trip.liability.billed_metres)?,
            to_db(trip.liability.incl_gst_micros)?,
            to_db(trip.liability.excl_gst_micros)?,
            to_db(trip.liability.gst_micros)?,
            owner,
            trip.vehicle_id,
            owner_key(owner),
        ],
    )?;
    Ok(())
}

/// The key that scopes trip IDs. Each user has their own namespace, and trips
/// recorded with the owner key share one called `owner`.
pub(crate) fn owner_key(owner: Option<&str>) -> &str {
    owner.unwrap_or("owner")
}

/// Reads one numeric query parameter, such as `limit=50`. Absent means `None`.
pub(crate) fn query_number(query: Option<&str>, name: &str) -> Result<Option<u32>, ApiError> {
    query_value(query, name)
        .map(|value| {
            value
                .parse()
                .map_err(|_| ApiError::BadRequest(format!("{name} must be a whole number")))
        })
        .transpose()
}

/// Converts a calculated amount to the database integer type.
fn to_db(value: u64) -> Result<i64, ApiError> {
    i64::try_from(value).map_err(|_| ApiError::Internal)
}

/// Returns the static type code from the rate table that matches `code`.
///
/// The core borrows type codes as `&'static str`, so the request's code is
/// swapped for the table's own copy.
pub(crate) fn known_type_code(code: &str) -> Option<&'static str> {
    NZTA_SCHEDULE
        .iter()
        .flat_map(|table| table.rules.iter())
        .find(|rule| rule.type_code == code)
        .map(|rule| rule.type_code)
}

fn describe_rate_error(error: RucError, date: RateDate) -> String {
    match error {
        RucError::NoRateTable => format!("no RUC rates are in force on {}", format_date(date)),
        RucError::NoMatchingRule => "no RUC rate matches this vehicle's type and weight".to_owned(),
        RucError::InvalidVehicle => "vehicle is not valid for RUC".to_owned(),
        RucError::InvalidDistance => "metres must be a non-negative number".to_owned(),
        RucError::Overflow => "the charge is too large to calculate".to_owned(),
    }
}

/// Parses `YYYY-MM-DD`, rejecting dates that do not exist, such as 2026-02-30.
pub fn parse_date(text: &str) -> Option<RateDate> {
    let mut parts = text.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let digits =
        |part: &str, width: usize| part.len() == width && part.bytes().all(|b| b.is_ascii_digit());
    if !(digits(year, 4) && digits(month, 2) && digits(day, 2)) {
        return None;
    }
    let date = RateDate {
        year: year.parse().ok()?,
        month: month.parse().ok()?,
        day: day.parse().ok()?,
    };
    if !(1..=12).contains(&date.month) || date.day == 0 || date.day > days_in_month(date) {
        return None;
    }
    Some(date)
}

/// Formats a date as `YYYY-MM-DD`.
pub fn format_date(date: RateDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

fn days_in_month(date: RateDate) -> u8 {
    match date.month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(date.year) => 29,
        2 => 28,
        _ => 31,
    }
}

fn is_leap_year(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_trip() -> TripInput {
        TripInput {
            id: "trip-1".to_owned(),
            date: "2026-10-01".to_owned(),
            metres: 1_000.0,
            vehicle_id: None,
            vehicle: Some(VehicleInput {
                rate_type: "1".to_owned(),
                gross_kg: 3_000,
                electric: false,
            }),
        }
    }

    #[test]
    fn accepts_real_dates_only() {
        assert_eq!(
            parse_date("2026-08-06"),
            Some(RateDate {
                year: 2026,
                month: 8,
                day: 6
            })
        );
        assert_eq!(
            parse_date("2024-02-29"),
            Some(RateDate {
                year: 2024,
                month: 2,
                day: 29
            })
        );
        assert_eq!(parse_date("2026-02-29"), None);
        assert_eq!(parse_date("2026-13-01"), None);
        assert_eq!(parse_date("2026-00-10"), None);
        assert_eq!(parse_date("2026-4-1"), None);
        assert_eq!(parse_date("+026-04-01"), None);
        assert_eq!(parse_date("2026-04-01-01"), None);
    }

    #[test]
    fn dates_round_trip_through_text() {
        let date = parse_date("2026-09-30").unwrap();
        assert_eq!(format_date(date), "2026-09-30");
    }

    #[test]
    fn unknown_type_codes_are_not_mapped() {
        assert!(known_type_code("no-such-code").is_none());
        assert!(known_type_code("1").is_some());
    }

    #[test]
    fn query_numbers_are_read_by_name() {
        assert_eq!(
            query_number(Some("limit=5&offset=2"), "offset").unwrap(),
            Some(2)
        );
        assert_eq!(query_number(Some("limitx=5"), "limit").unwrap(), None);
        assert_eq!(query_number(None, "limit").unwrap(), None);
        assert!(query_number(Some("limit=five"), "limit").is_err());
    }

    #[test]
    fn common_checks_reject_bad_input_with_a_message() {
        assert!(check_common(&base_trip()).is_ok());
        let negative = TripInput {
            metres: -1.0,
            ..base_trip()
        };
        assert_eq!(
            check_common(&negative).unwrap_err(),
            "metres must be a non-negative number"
        );
    }

    #[test]
    fn inline_vehicle_is_charged_with_its_own_fields() {
        let input = base_trip();
        let trip = charge(
            &input,
            parse_date("2026-10-01").unwrap(),
            1_000.0,
            "1",
            3_000,
            false,
            None,
        )
        .unwrap();
        assert_eq!(trip.rate_type, "1");
        assert_eq!(trip.vehicle_id, None);
    }
}
