// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! JavaScript bindings for `tpt-ruc-core`, used by the PWA.
//!
//! Inputs and outputs are plain values or JSON strings, so the PWA can call
//! the same maths as the device and backend. Money is returned in
//! micro-dollars, as in the core crate.

use serde::Serialize;
use tpt_ruc_core::licence::DistanceLicence;
use tpt_ruc_core::ruc::{self, RateDate, Vehicle};
use tpt_ruc_core::ruc_rates::{self, PurchaseMethod};
use tpt_ruc_core::{billable_km, haversine_m as core_haversine_m, Coordinate, Fix, Tracker};
use wasm_bindgen::prelude::*;

/// Great-circle distance between two points, in metres.
#[wasm_bindgen(js_name = haversineM)]
pub fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> Result<f64, JsError> {
    let a = Coordinate::new(lat1, lon1).map_err(|e| JsError::new(&e.to_string()))?;
    let b = Coordinate::new(lat2, lon2).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(core_haversine_m(a, b))
}

/// Billable kilometres for a total distance in metres.
#[wasm_bindgen(js_name = billableKm)]
pub fn billable_km_js(metres: f64) -> f64 {
    billable_km(metres)
}

/// Effective date of the newest NZTA rate table, as `YYYY-MM-DD`.
#[wasm_bindgen(js_name = currentRateEffective)]
pub fn current_rate_effective() -> String {
    let table = ruc_rates::NZTA_SCHEDULE
        .last()
        .expect("the schedule always has a table");
    let date = table.effective;
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

/// The date a heavy licence bought under the rate change on `year-month-day`
/// expires, as `YYYY-MM-DD`. `undefined` for vehicles at or under the heavy limit.
#[wasm_bindgen(js_name = heavyLicenceExpiresOn)]
pub fn heavy_licence_expires_on_js(gross_kg: u32, year: u16, month: u8, day: u8) -> Option<String> {
    ruc::heavy_licence_expires_on(gross_kg, RateDate { year, month, day })
        .map(|date| format!("{:04}-{:02}-{:02}", date.year, date.month, date.day))
}

/// Kilometres left on a distance licence bought at `start_odometer_km` for `licensed_km`,
/// at the odometer reading `odometer_km`. Zero once the licence distance is used up.
///
/// The core model is marked requires verification, so the PWA must say the figure is an estimate.
#[wasm_bindgen(js_name = licenceRemainingKm)]
pub fn licence_remaining_km(start_odometer_km: u32, licensed_km: u32, odometer_km: u32) -> u32 {
    let licence = DistanceLicence {
        start_odometer_km: u64::from(start_odometer_km),
        licensed_km: u64::from(licensed_km),
    };
    // The remaining distance is at most `licensed_km`, which fits in u32.
    licence.remaining_km(u64::from(odometer_km)) as u32
}

/// Standard RUC liability from the NZTA table in force on the given date.
///
/// Returns `{ rateVersion, billedMetres, inclGstMicros, exclGstMicros, gstMicros }`.
#[wasm_bindgen(js_name = liability)]
pub fn liability(
    rate_type: &str,
    gross_kg: u32,
    electric: bool,
    year: u16,
    month: u8,
    day: u8,
    metres: f64,
) -> Result<JsValue, JsError> {
    // The core crate needs a `'static` type code, so match it against the table.
    let code = ruc_rates::NZTA_2026_08_06
        .rules
        .iter()
        .map(|r| r.type_code)
        .find(|code| *code == rate_type)
        .ok_or_else(|| JsError::new("unknown RUC type code"))?;
    let vehicle = Vehicle {
        rate_type: code,
        gross_kg,
        electric,
    };
    let l = ruc::liability(
        ruc_rates::NZTA_SCHEDULE,
        &vehicle,
        RateDate { year, month, day },
        metres,
    )
    .map_err(|e| JsError::new(&format!("{e:?}")))?;
    to_js(&LiabilityOut {
        rate_version: l.rate_version,
        billed_metres: l.billed_metres,
        incl_gst_micros: l.incl_gst_micros,
        excl_gst_micros: l.excl_gst_micros,
        gst_micros: l.gst_micros,
    })
}

/// Administration fee for one licence, including GST, in micro-dollars.
#[wasm_bindgen(js_name = adminFeeInclGstMicros)]
pub fn admin_fee_incl_gst_micros(agent: bool) -> u64 {
    let method = if agent {
        PurchaseMethod::Agent
    } else {
        PurchaseMethod::Online
    };
    ruc_rates::admin_fee_incl_gst_micros(method)
}

/// Parses one NMEA 0183 sentence, such as `$GNGGA,...*47`.
///
/// Returns the sentence as a serde object keyed by its variant: `{ Gga: {...} }`,
/// `{ Rmc: {...} }`, `{ Vtg: {...} }`, `{ Gsa: {...} }`, or `"Unsupported"` for a
/// valid sentence this parser does not decode. Fails if the checksum or fields
/// are invalid.
#[wasm_bindgen(js_name = parseNmea)]
pub fn parse_nmea(line: &str) -> Result<JsValue, JsError> {
    let sentence = tpt_ruc_core::parse_sentence(line).map_err(|e| JsError::new(&e.to_string()))?;
    to_js(&sentence)
}

/// A trip tracker that filters GPS fixes and accumulates distance.
#[wasm_bindgen]
pub struct TripTracker {
    inner: Tracker,
}

#[wasm_bindgen]
impl TripTracker {
    /// Creates a tracker with the default thresholds.
    #[wasm_bindgen(constructor)]
    pub fn new() -> TripTracker {
        TripTracker {
            inner: Tracker::new(tpt_ruc_core::Config::default()),
        }
    }

    /// Feeds one fix, given as the JSON form of the core `Fix` type.
    ///
    /// Returns `{ status: "accepted", metres, closed }` or
    /// `{ status: "rejected", reason }`. `closed` is a finished segment, if a
    /// gap closed one.
    pub fn push(&mut self, fix_json: &str) -> Result<JsValue, JsError> {
        let fix: Fix = serde_json::from_str(fix_json).map_err(|e| JsError::new(&e.to_string()))?;
        let outcome = match self.inner.push(fix) {
            Ok(accepted) => PushOutcome::Accepted {
                metres: accepted.metres,
                closed: accepted.closed,
            },
            Err(reason) => PushOutcome::Rejected {
                reason: format!("{reason:?}"),
            },
        };
        to_js(&outcome)
    }

    /// Total accepted distance so far, in metres.
    #[wasm_bindgen(js_name = totalM)]
    pub fn total_m(&self) -> f64 {
        self.inner.total_m()
    }
}

impl Default for TripTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LiabilityOut {
    rate_version: &'static str,
    billed_metres: u64,
    incl_gst_micros: u64,
    excl_gst_micros: u64,
    gst_micros: u64,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum PushOutcome {
    Accepted {
        metres: f64,
        closed: Option<tpt_ruc_core::Segment>,
    },
    Rejected {
        reason: String,
    },
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}
