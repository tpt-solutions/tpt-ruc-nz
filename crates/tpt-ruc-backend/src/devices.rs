// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Device keys and signed trip uploads.
//!
//! A device (for example a GNSS unit or a phone app) holds an Ed25519 key pair.
//! Its public key is registered against a user. A trip upload made with the
//! device signs the trips, so the server can check that the upload came from
//! that device and was not changed on the way.
//!
//! A signed upload sends three headers with the usual user token:
//!
//! - `X-Device-Id`: the device's ID, as returned on registration.
//! - `X-Device-Sequence`: a number that must rise with every upload from the
//!   device. A repeated or older number is refused, which stops replays.
//! - `X-Device-Signature`: 64 bytes, hex encoded. It signs the message built by
//!   [`signing_message`].
//!
//! Uploads without these headers are unchanged. They use the user token alone.

use bytes::Bytes;
use http::{HeaderMap, Response, StatusCode};
use http_body_util::Full;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;

use crate::auth::{random_hex, Principal, Role};
use crate::error::ApiError;
use crate::{json_response, AppState};
use ed25519_dalek::{Signature, VerifyingKey};

/// Header carrying the device ID.
pub const HEADER_ID: &str = "x-device-id";
/// Header carrying the upload's sequence number.
pub const HEADER_SEQUENCE: &str = "x-device-sequence";
/// Header carrying the hex-encoded signature.
pub const HEADER_SIGNATURE: &str = "x-device-signature";

/// Domain separator, so a signature made for trips cannot be reused for anything else.
const MESSAGE_PREFIX: &str = "tpt-ruc-trips-v1";

/// The device details a signed upload needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedUpload {
    /// The device that signed the upload.
    pub device_id: String,
    /// The upload's sequence number.
    pub sequence: u64,
    /// The signature bytes.
    pub signature: [u8; 64],
}

/// Reads the device headers. Returns `None` when none are present.
///
/// If some but not all of the three headers are present, the upload is refused,
/// because a partly signed upload cannot be trusted.
pub fn upload_from_headers(headers: &HeaderMap) -> Result<Option<SignedUpload>, ApiError> {
    let id = headers.get(HEADER_ID);
    let sequence = headers.get(HEADER_SEQUENCE);
    let signature = headers.get(HEADER_SIGNATURE);
    if id.is_none() && sequence.is_none() && signature.is_none() {
        return Ok(None);
    }
    let (Some(id), Some(sequence), Some(signature)) = (id, sequence, signature) else {
        return Err(ApiError::BadRequest(
            "a device upload needs X-Device-Id, X-Device-Sequence and X-Device-Signature"
                .to_owned(),
        ));
    };
    let device_id = id
        .to_str()
        .ok()
        .filter(|value| !value.is_empty() && value.len() <= 64)
        .ok_or_else(|| ApiError::BadRequest("X-Device-Id is not valid".to_owned()))?
        .to_owned();
    let sequence: u64 = sequence
        .to_str()
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| {
            ApiError::BadRequest("X-Device-Sequence must be a whole number".to_owned())
        })?;
    let signature = signature
        .to_str()
        .ok()
        .and_then(from_hex)
        .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
        .ok_or(ApiError::Unauthorized)?;
    Ok(Some(SignedUpload {
        device_id,
        sequence,
        signature,
    }))
}

/// The bytes a device signs: a fixed prefix, the device ID, the sequence number, and the
/// trips as compact JSON with keys in sorted order.
pub fn signing_message(device_id: &str, sequence: u64, trips_json: &str) -> Vec<u8> {
    format!("{MESSAGE_PREFIX}\n{device_id}\n{sequence}\n{trips_json}").into_bytes()
}

/// A registered device, as the upload check reads it.
pub struct Device {
    /// The user the device belongs to.
    pub user_id: String,
    /// The device's public key.
    pub key: VerifyingKey,
}

/// Loads a device and checks the signature over `trips_json`.
///
/// Fails with `Unauthorized` for an unknown or revoked device, and for a signature
/// that does not verify. The sequence number is checked when the trips are stored,
/// in the same transaction, so two uploads cannot both use one number.
pub fn verify_upload(
    connection: &Connection,
    upload: &SignedUpload,
    trips_json: &str,
) -> Result<Device, ApiError> {
    let row: Option<(String, String)> = connection
        .query_row(
            "SELECT user_id, public_key FROM devices WHERE id = ?1 AND revoked = 0",
            params![upload.device_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((user_id, public_key)) = row else {
        return Err(ApiError::Unauthorized);
    };
    let key = from_hex(&public_key)
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
        .ok_or(ApiError::Internal)?;
    let signature = Signature::from_bytes(&upload.signature);
    let message = signing_message(&upload.device_id, upload.sequence, trips_json);
    key.verify_strict(&message, &signature)
        .map_err(|_| ApiError::Unauthorized)?;
    Ok(Device { user_id, key })
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    name: String,
    public_key: String,
    #[serde(default)]
    user_id: Option<String>,
}

/// `POST /v1/devices`: registers a device's public key against a user.
///
/// A driver registers devices for themselves only. Managers and admins may
/// register for any user. The owner key must name the user.
pub async fn register(
    state: &AppState,
    principal: &Principal,
    body: &Bytes,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let request: RegisterRequest = serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    let name = request.name.trim().to_owned();
    if name.is_empty() || name.len() > 100 {
        return Err(ApiError::BadRequest(
            "name must be 1 to 100 characters".to_owned(),
        ));
    }
    let key_bytes = from_hex(&request.public_key)
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .ok_or_else(|| ApiError::BadRequest("public_key must be 64 hex characters".to_owned()))?;
    VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| ApiError::BadRequest("public_key is not a valid Ed25519 key".to_owned()))?;
    let public_key = request.public_key.to_ascii_lowercase();

    let user_id = match (principal.role(), request.user_id, principal.user_id()) {
        (Role::Driver, Some(other), Some(me)) if other != me => return Err(ApiError::Forbidden),
        (Role::Driver, _, Some(me)) => me.to_owned(),
        (_, Some(user), _) => user,
        (_, None, Some(me)) => me.to_owned(),
        (_, None, None) => {
            return Err(ApiError::BadRequest(
                "user_id is required when using the owner key".to_owned(),
            ))
        }
    };

    let id = format!("d_{}", random_hex(12)?);
    {
        let (id, user_id, name, public_key) =
            (id.clone(), user_id.clone(), name.clone(), public_key);
        state
            .db
            .run(move |connection| {
                let known: bool = connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM users WHERE id = ?1)",
                    params![user_id],
                    |row| row.get(0),
                )?;
                if !known {
                    return Err(ApiError::BadRequest("unknown user_id".to_owned()));
                }
                connection.execute(
                    "INSERT INTO devices (id, user_id, name, public_key) VALUES (?1, ?2, ?3, ?4)",
                    params![id, user_id, name, public_key],
                )?;
                Ok(())
            })
            .await?;
    }
    Ok(json_response(
        StatusCode::CREATED,
        &json!({ "id": id, "user_id": user_id, "name": name }),
    ))
}

/// `GET /v1/devices`: lists devices. Drivers see their own.
pub async fn list(
    state: &AppState,
    principal: &Principal,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let scope = principal.trip_scope().map(str::to_owned);
    let devices = state
        .db
        .run(move |connection| {
            let mut statement = connection.prepare(
                "SELECT id, user_id, name, last_sequence, revoked, created_at FROM devices \
                 WHERE (?1 IS NULL OR user_id = ?1) ORDER BY created_at, id",
            )?;
            let rows = statement.query_map(params![scope], |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "user_id": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                    "last_sequence": row.get::<_, i64>(3)?,
                    "revoked": row.get::<_, bool>(4)?,
                    "created_at": row.get::<_, String>(5)?,
                }))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(json_response(
        StatusCode::OK,
        &json!({ "devices": devices }),
    ))
}

/// `DELETE /v1/devices/{id}`: revokes a device so its uploads are refused.
///
/// Managers and admins may revoke any device. A driver may revoke only their own.
pub async fn revoke(
    state: &AppState,
    principal: &Principal,
    id: &str,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let scope = principal.trip_scope().map(str::to_owned);
    let id = id.to_owned();
    let changed = state
        .db
        .run(move |connection| {
            Ok(connection.execute(
                "UPDATE devices SET revoked = 1 WHERE id = ?1 AND (?2 IS NULL OR user_id = ?2)",
                params![id, scope],
            )?)
        })
        .await?;
    if changed == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(json_response(StatusCode::OK, &json!({ "revoked": true })))
}

/// Decodes lowercase or uppercase hex. Returns `None` for odd length or other characters.
pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_rejects_bad_input() {
        assert_eq!(from_hex("00ff7A"), Some(vec![0x00, 0xff, 0x7a]));
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
    }

    #[test]
    fn the_message_binds_device_sequence_and_trips() {
        let a = signing_message("d_1", 7, "[]");
        assert_ne!(a, signing_message("d_1", 8, "[]"));
        assert_ne!(a, signing_message("d_2", 7, "[]"));
        assert_ne!(a, signing_message("d_1", 7, "[1]"));
    }

    #[test]
    fn headers_are_all_or_nothing() {
        let none = HeaderMap::new();
        assert_eq!(upload_from_headers(&none).unwrap(), None);

        let mut partial = HeaderMap::new();
        partial.insert(HEADER_ID, "d_1".parse().unwrap());
        assert!(upload_from_headers(&partial).is_err());
    }
}
