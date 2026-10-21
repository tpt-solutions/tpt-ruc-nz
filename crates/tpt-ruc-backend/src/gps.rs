// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Raw GPS points, uploaded only when the owner has turned on
//! `TPT_RUC_ALLOW_RAW_GPS`.
//!
//! The default is summaries only: trips carry a distance and a charge, and no
//! positions. A point is stored for the user who sent it, and deleting that user
//! deletes their points. The owner key has no user record, so it cannot upload.

use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::Full;
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;

use crate::auth::Principal;
use crate::error::ApiError;
use crate::{json_response, AppState};

/// Most points accepted in one upload. A request body is capped at 256 KiB, so this
/// is the tighter limit in practice.
pub const MAX_POINTS: usize = 1000;

/// Highest speed accepted, in km/h. Anything faster is a bad fix, not a vehicle.
const MAX_SPEED_KMH: f64 = 400.0;

#[derive(Debug, Deserialize)]
struct Batch {
    points: Vec<Point>,
}

#[derive(Debug, Deserialize)]
struct Point {
    recorded_at: String,
    lat: f64,
    lon: f64,
    speed_kmh: Option<f64>,
}

/// `POST /v1/gps/points`: stores a batch of raw points for the signed-in user.
/// The whole batch is refused if any point is invalid, so nothing is half-stored.
pub async fn upload(
    state: &AppState,
    principal: &Principal,
    body: &Bytes,
) -> Result<Response<Full<Bytes>>, ApiError> {
    if !state.allow_raw_gps {
        return Err(ApiError::Forbidden);
    }
    let user_id = principal.user_id().ok_or(ApiError::Forbidden)?.to_owned();

    let batch: Batch = serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    if batch.points.is_empty() || batch.points.len() > MAX_POINTS {
        return Err(ApiError::BadRequest(format!(
            "points must hold 1 to {MAX_POINTS} entries"
        )));
    }
    for (index, point) in batch.points.iter().enumerate() {
        validate(point)
            .map_err(|reason| ApiError::BadRequest(format!("point {index}: {reason}")))?;
    }

    let stored = batch.points.len();
    state
        .db
        .run(move |connection| {
            let transaction = connection.transaction()?;
            for point in &batch.points {
                transaction.execute(
                    "INSERT INTO raw_points (user_id, recorded_at, lat, lon, speed_kmh)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        user_id,
                        point.recorded_at,
                        point.lat,
                        point.lon,
                        point.speed_kmh
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .await?;

    Ok(json_response(
        StatusCode::CREATED,
        &json!({ "stored": stored }),
    ))
}

/// Checks one point. Returns the reason when it is refused.
fn validate(point: &Point) -> Result<(), String> {
    if !valid_timestamp(&point.recorded_at) {
        return Err("recorded_at must be a UTC time such as 2026-10-22T09:30:00Z".to_owned());
    }
    if !point.lat.is_finite() || !(-90.0..=90.0).contains(&point.lat) {
        return Err("lat must be between -90 and 90".to_owned());
    }
    if !point.lon.is_finite() || !(-180.0..=180.0).contains(&point.lon) {
        return Err("lon must be between -180 and 180".to_owned());
    }
    if let Some(speed) = point.speed_kmh {
        if !speed.is_finite() || !(0.0..=MAX_SPEED_KMH).contains(&speed) {
            return Err(format!("speed_kmh must be between 0 and {MAX_SPEED_KMH}"));
        }
    }
    Ok(())
}

/// Accepts `YYYY-MM-DDTHH:MM:SSZ`, with optional milliseconds. Text order then matches time order.
fn valid_timestamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    let shape: &[u8] = match bytes.len() {
        20 => b"dddd-dd-ddTdd:dd:ddZ",
        24 => b"dddd-dd-ddTdd:dd:dd.dddZ",
        _ => return false,
    };
    bytes
        .iter()
        .zip(shape)
        .all(|(byte, expected)| match expected {
            b'd' => byte.is_ascii_digit(),
            other => byte == other,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_must_be_utc_with_the_fixed_shape() {
        assert!(valid_timestamp("2026-10-22T09:30:00Z"));
        assert!(valid_timestamp("2026-10-22T09:30:00.250Z"));
        assert!(!valid_timestamp("2026-10-22 09:30:00Z"));
        assert!(!valid_timestamp("2026-10-22T09:30:00"));
        assert!(!valid_timestamp("2026-10-22T09:30:00+12:00"));
        assert!(!valid_timestamp("yesterday"));
    }

    #[test]
    fn coordinates_and_speed_are_range_checked() {
        let good = Point {
            recorded_at: "2026-10-22T09:30:00Z".to_owned(),
            lat: -41.3,
            lon: 174.8,
            speed_kmh: Some(62.0),
        };
        assert!(validate(&good).is_ok());
        assert!(validate(&Point {
            lat: 91.0,
            ..good_copy(&good)
        })
        .is_err());
        assert!(validate(&Point {
            lon: f64::NAN,
            ..good_copy(&good)
        })
        .is_err());
        assert!(validate(&Point {
            speed_kmh: Some(-1.0),
            ..good_copy(&good)
        })
        .is_err());
        assert!(validate(&Point {
            speed_kmh: None,
            ..good_copy(&good)
        })
        .is_ok());
    }

    fn good_copy(point: &Point) -> Point {
        Point {
            recorded_at: point.recorded_at.clone(),
            lat: point.lat,
            lon: point.lon,
            speed_kmh: point.speed_kmh,
        }
    }
}
