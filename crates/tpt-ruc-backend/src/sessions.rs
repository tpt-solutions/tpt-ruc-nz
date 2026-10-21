// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Browser sign-in with a user ID and password, which issues a session token.
//!
//! A session is a second way to hold a user's identity. It lasts
//! [`SESSION_HOURS`] from sign-in and then stops working. Signing out, or
//! changing the user's password, ends every session at once. API tokens are
//! unaffected by sessions and keep working until they are rotated.

use std::sync::OnceLock;

use bytes::Bytes;
use http::{HeaderMap, Response, StatusCode};
use http_body_util::Full;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::json;

use crate::auth::{bearer, random_hex, token_hash};
use crate::error::ApiError;
use crate::{json_response, passwords, AppState};

/// How long a session lasts after sign-in, in hours.
pub const SESSION_HOURS: u32 = 12;

#[derive(Debug, Deserialize)]
struct LoginRequest {
    user_id: String,
    password: String,
}

/// `POST /v1/sessions`: signs in with a user ID and password. Returns a session token, shown once.
pub async fn login(state: &AppState, body: &Bytes) -> Result<Response<Full<Bytes>>, ApiError> {
    let request: LoginRequest = serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    let user_id = request.user_id.trim().to_owned();
    let password = request.password;

    let found = {
        let user_id = user_id.clone();
        state
            .db
            .run(move |connection| {
                Ok(connection
                    .query_row(
                        "SELECT id, name, role, password_hash FROM users WHERE id = ?1",
                        params![user_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, Option<String>>(3)?,
                            ))
                        },
                    )
                    .optional()?)
            })
            .await?
    };

    // Check a password even when the user is unknown or has none, so the
    // response time does not reveal which user IDs exist.
    let (user, stored) = match found {
        Some((id, name, role, Some(stored))) => ((id, name, role), stored),
        _ => {
            passwords::verify(&password, dummy_hash());
            return Err(ApiError::Unauthorized);
        }
    };
    if !passwords::verify(&password, &stored) {
        return Err(ApiError::Unauthorized);
    }

    let token = format!("tps_{}", random_hex(32)?);
    let session_id = format!("s_{}", random_hex(12)?);
    let hash = token_hash(&token);
    let expires_at = {
        let user_id = user.0.clone();
        state
            .db
            .run(move |connection| {
                connection.execute(
                    "INSERT INTO sessions (id, user_id, token_hash, expires_at)
                     VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?4))",
                    params![session_id, user_id, hash, format!("+{SESSION_HOURS} hours")],
                )?;
                Ok(connection.query_row(
                    "SELECT expires_at FROM sessions WHERE token_hash = ?1",
                    params![hash],
                    |row| row.get::<_, String>(0),
                )?)
            })
            .await?
    };

    tracing::info!(user = %user.0, "session started");
    Ok(json_response(
        StatusCode::CREATED,
        &json!({
            "token": token,
            "expires_at": expires_at,
            "user": { "id": user.0, "name": user.1, "role": user.2 },
            "note": "Store this token now. It cannot be shown again.",
        }),
    ))
}

/// `DELETE /v1/sessions`: ends the session named by the bearer token. Other tokens are left alone.
pub async fn logout(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let token = bearer(headers).ok_or(ApiError::Unauthorized)?;
    let hash = token_hash(token);
    let ended = state
        .db
        .run(move |connection| {
            Ok(connection.execute("DELETE FROM sessions WHERE token_hash = ?1", params![hash])?)
        })
        .await?;
    // A token that matches no session is still a valid API token, so the owner key or a user token gets 200 with `ended` false.
    Ok(json_response(
        StatusCode::OK,
        &json!({ "signed_out": ended > 0 }),
    ))
}

/// An Argon2 hash that matches no real password. Used only to spend the same time on an unknown user.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        passwords::hash("unused-placeholder-password").unwrap_or_else(|_| {
            // Falls back to a well-formed hash that verifies nothing.
            "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned()
        })
    })
}
