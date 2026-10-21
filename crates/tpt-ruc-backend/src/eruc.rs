// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Generic eRUC export and the adapter trait that real providers will implement.
//!
//! `GET /v1/eruc/export` builds one batch of stored trips for a date range and
//! turns it into the payload of the chosen provider. The generic providers,
//! `generic-json` and `generic-csv`, are fixed formats documented in
//! `docs/eruc-export.md`. A provider adapter maps the same batch to its own
//! format. Nothing here sends data anywhere: pushing to a provider is a
//! separate job, and waits for provider API documentation.
//!
//! Distances and money use the same decimal text as the NZTA CSV reports, so
//! the two outputs agree to the last digit.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use http::{header, HeaderValue, Response, StatusCode};
use http_body_util::Full;
use rusqlite::params;
use serde::Serialize;

use crate::error::ApiError;
use crate::reports::{csv_field, data_line, date_range, header_line, km, nzd};
use crate::trips::{trip_row, TripRow, TRIP_COLUMNS};
use crate::{query_value, AppState};

/// Identifies the export, so a reader can tell what the file is.
pub const ERUC_FORMAT: &str = "tpt-ruc-eruc";
/// Version of the export layout. Bump it when a field changes meaning.
pub const ERUC_VERSION: u32 = 1;
/// Provider used when the query names none.
pub const DEFAULT_PROVIDER: &str = "generic-json";

/// One trip, as an eRUC provider receives it.
///
/// The recording user is left out, so a provider never sees who drove. The
/// vehicle is named by its registered ID, or is `None` for an inline vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErucRecord {
    /// The trip's ID, as the device or app recorded it.
    pub trip_id: String,
    /// The trip date, `YYYY-MM-DD`.
    pub trip_date: String,
    /// The registered vehicle, or `None` for an inline vehicle.
    pub vehicle_id: Option<String>,
    /// The NZTA RUC type code the trip was charged under.
    pub vehicle_type: String,
    /// Gross vehicle weight in kilograms.
    pub gross_kg: i64,
    /// Whether the vehicle is electric.
    pub electric: bool,
    /// Billed distance in kilometres, to three decimal places.
    pub distance_km: String,
    /// The rate table version the charge was calculated with.
    pub rate_version: String,
    /// The charge before GST, in NZD to the cent.
    pub charge_excl_gst_nzd: String,
    /// The GST included in the charge, in NZD to the cent.
    pub gst_nzd: String,
    /// The charge including GST, in NZD to the cent.
    pub charge_incl_gst_nzd: String,
}

/// Everything in one export: the header fields and the records, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErucBatch {
    /// [`ERUC_FORMAT`].
    pub format: &'static str,
    /// [`ERUC_VERSION`].
    pub version: u32,
    /// First date covered, inclusive.
    pub from: String,
    /// Last date covered, inclusive.
    pub to: String,
    /// The trips in the range, oldest first.
    pub records: Vec<ErucRecord>,
}

/// An adapter that turns a batch into one provider's payload.
pub trait ErucProvider: Sync {
    /// The name a caller passes as `provider=` in the query string.
    fn id(&self) -> &'static str;
    /// The media type of [`ErucProvider::payload`].
    fn content_type(&self) -> &'static str;
    /// The file extension used in the download name, without the dot.
    fn extension(&self) -> &'static str;
    /// Builds the payload for `batch`.
    fn payload(&self, batch: &ErucBatch) -> Vec<u8>;
}

/// The generic export as one JSON document.
pub struct GenericJson;

impl ErucProvider for GenericJson {
    fn id(&self) -> &'static str {
        "generic-json"
    }

    fn content_type(&self) -> &'static str {
        "application/json"
    }

    fn extension(&self) -> &'static str {
        "json"
    }

    fn payload(&self, batch: &ErucBatch) -> Vec<u8> {
        serde_json::to_vec_pretty(batch).unwrap_or_else(|_| b"{}".to_vec())
    }
}

/// The generic export as CSV, one line per trip, with a header line.
pub struct GenericCsv;

/// Column names of the generic CSV, in order. `docs/eruc-export.md` lists the same names.
const CSV_COLUMNS: &str = "trip_id,trip_date,vehicle_id,vehicle_type,gross_kg,electric,\
    distance_km,rate_version,charge_excl_gst_nzd,gst_nzd,charge_incl_gst_nzd";

impl ErucProvider for GenericCsv {
    fn id(&self) -> &'static str {
        "generic-csv"
    }

    fn content_type(&self) -> &'static str {
        "text/csv; charset=utf-8"
    }

    fn extension(&self) -> &'static str {
        "csv"
    }

    fn payload(&self, batch: &ErucBatch) -> Vec<u8> {
        let mut csv = header_line(CSV_COLUMNS);
        for record in &batch.records {
            csv.push_str(&data_line(&[
                csv_field(&record.trip_id),
                csv_field(&record.trip_date),
                csv_field(record.vehicle_id.as_deref().unwrap_or("")),
                csv_field(&record.vehicle_type),
                record.gross_kg.to_string(),
                record.electric.to_string(),
                record.distance_km.clone(),
                csv_field(&record.rate_version),
                record.charge_excl_gst_nzd.clone(),
                record.gst_nzd.clone(),
                record.charge_incl_gst_nzd.clone(),
            ]));
        }
        csv.into_bytes()
    }
}

/// Every provider this server can build an export for.
pub const PROVIDERS: &[&dyn ErucProvider] = &[&GenericJson, &GenericCsv];

/// Finds a provider by its `id`.
pub fn provider_named(id: &str) -> Option<&'static dyn ErucProvider> {
    PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.id() == id)
}

/// Maps a stored trip to an eRUC record.
pub fn record_from_trip(trip: &TripRow) -> ErucRecord {
    ErucRecord {
        trip_id: trip.id.clone(),
        trip_date: trip.trip_date.clone(),
        vehicle_id: trip.vehicle_id.clone(),
        vehicle_type: trip.rate_type.clone(),
        gross_kg: trip.gross_kg,
        electric: trip.electric,
        distance_km: km(trip.billed_metres),
        rate_version: trip.rate_version.clone(),
        charge_excl_gst_nzd: nzd(trip.excl_gst_micros),
        gst_nzd: nzd(trip.gst_micros),
        charge_incl_gst_nzd: nzd(trip.incl_gst_micros),
    }
}

/// `GET /v1/eruc/export?from=&to=&provider=`: the trips in the inclusive range, in the provider's format.
///
/// Managers and admins see every trip, so this endpoint is manager-only, like
/// the reports. An unknown provider is a bad request.
pub async fn export(
    state: &AppState,
    query: Option<&str>,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let range = date_range(query)?;
    let provider_id = query_value(query, "provider").unwrap_or(DEFAULT_PROVIDER);
    let provider = provider_named(provider_id)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown provider: {provider_id}")))?;

    let batch = load_batch(state, range.from, range.to).await?;
    Ok(payload_response(provider, &batch))
}

/// Builds the batch of every stored trip dated from `from` to `to`, inclusive.
pub async fn load_batch(state: &AppState, from: String, to: String) -> Result<ErucBatch, ApiError> {
    let (first, last) = (from.clone(), to.clone());
    let trips = state
        .db
        .run(move |connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {TRIP_COLUMNS} FROM trips WHERE trip_date BETWEEN ?1 AND ?2                  ORDER BY trip_date, id"
            ))?;
            let rows = statement.query_map(params![first, last], trip_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;

    Ok(ErucBatch {
        format: ERUC_FORMAT,
        version: ERUC_VERSION,
        from,
        to,
        records: trips.iter().map(record_from_trip).collect(),
    })
}

/// The result of a push: `Ok` with a short note for the audit log, or an error.
pub type PushResult = Result<String, PushError>;

/// Why a push failed. A retryable failure is tried again later, with a delay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushError {
    /// Worth trying again later: the network, a busy server, a timeout.
    Retryable(String),
    /// Will not succeed as sent: the provider refused the data, or the credentials are wrong.
    Permanent(String),
}

/// A boxed future, so push adapters can be used behind a trait object.
pub type PushFuture<'a> = Pin<Box<dyn Future<Output = PushResult> + Send + 'a>>;

/// Sends a batch to a provider. Real adapters will make the network calls here.
///
/// Nothing in the server calls a provider unless `TPT_RUC_ERUC_PROVIDER` names
/// one, so the default is to send nothing.
pub trait PushProvider: Send + Sync {
    /// The name used in `TPT_RUC_ERUC_PROVIDER` and in job requests.
    fn id(&self) -> &'static str;
    /// Sends `batch`, or reports why it could not be sent.
    fn push<'a>(&'a self, batch: &'a ErucBatch) -> PushFuture<'a>;
}

/// The placeholder provider. It sends nothing, and says so in the audit log.
///
/// It exercises the queue, retries and audit log without a real provider. Its
/// name is `dry-run`.
pub struct DryRun;

impl PushProvider for DryRun {
    fn id(&self) -> &'static str {
        "dry-run"
    }

    fn push<'a>(&'a self, batch: &'a ErucBatch) -> PushFuture<'a> {
        let count = batch.records.len();
        Box::pin(async move { Ok(format!("dry run: {count} records not sent")) })
    }
}

/// Finds a push provider by its `id`, for the names this build knows.
pub fn push_provider_named(id: &str) -> Option<Arc<dyn PushProvider>> {
    match id {
        "dry-run" => Some(Arc::new(DryRun)),
        _ => None,
    }
}

/// Wraps a provider's payload in a download response.
fn payload_response(provider: &dyn ErucProvider, batch: &ErucBatch) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from(provider.payload(batch))));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(provider.content_type()),
    );
    let disposition = format!(
        "attachment; filename=\"eruc-{}-{}-{}.{}\"",
        provider.id(),
        batch.from,
        batch.to,
        provider.extension()
    );
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str) -> ErucRecord {
        ErucRecord {
            trip_id: id.to_owned(),
            trip_date: "2026-10-01".to_owned(),
            vehicle_id: None,
            vehicle_type: "1".to_owned(),
            gross_kg: 3000,
            electric: false,
            distance_km: "12.345".to_owned(),
            rate_version: "2026-08-06".to_owned(),
            charge_excl_gst_nzd: "9.91".to_owned(),
            gst_nzd: "1.49".to_owned(),
            charge_incl_gst_nzd: "11.40".to_owned(),
        }
    }

    fn batch(records: Vec<ErucRecord>) -> ErucBatch {
        ErucBatch {
            format: ERUC_FORMAT,
            version: ERUC_VERSION,
            from: "2026-10-01".to_owned(),
            to: "2026-10-31".to_owned(),
            records,
        }
    }

    #[test]
    fn providers_have_unique_ids() {
        assert!(provider_named("generic-json").is_some());
        assert!(provider_named("generic-csv").is_some());
        assert!(provider_named("nope").is_none());
        assert!(provider_named(DEFAULT_PROVIDER).is_some());
    }

    #[test]
    fn csv_has_header_and_one_line_per_record() {
        let payload = GenericCsv.payload(&batch(vec![record("t-1"), record("t-2")]));
        let text = String::from_utf8(payload).unwrap();
        let lines: Vec<&str> = text.split("\r\n").collect();
        assert_eq!(lines[0], CSV_COLUMNS);
        assert_eq!(
            lines[1],
            "t-1,2026-10-01,,1,3000,false,12.345,2026-08-06,9.91,1.49,11.40"
        );
        assert!(lines[2].starts_with("t-2,"));
        assert_eq!(
            lines.len(),
            4,
            "header, two records and the trailing empty split"
        );
    }

    #[test]
    fn json_carries_the_header_fields_and_records() {
        let payload = GenericJson.payload(&batch(vec![record("t-1")]));
        let value: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(value["format"], ERUC_FORMAT);
        assert_eq!(value["version"], ERUC_VERSION);
        assert_eq!(value["from"], "2026-10-01");
        assert_eq!(value["records"][0]["trip_id"], "t-1");
        assert_eq!(value["records"][0]["distance_km"], "12.345");
        assert!(value["records"][0].get("user_id").is_none());
    }

    #[test]
    fn csv_escapes_text_fields() {
        let mut odd = record("a,b");
        odd.vehicle_id = Some("=HYPERLINK()".to_owned());
        let text = String::from_utf8(GenericCsv.payload(&batch(vec![odd]))).unwrap();
        assert!(text.contains("\"a,b\",2026-10-01,'=HYPERLINK(),"));
    }
}
