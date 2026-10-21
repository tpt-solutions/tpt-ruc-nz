// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! User management. Only administrators reach these handlers.

use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::Full;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::{random_hex, token_hash, Role};
use crate::error::ApiError;
use crate::passwords;
use crate::{json_response, AppState};

/// Longest display name accepted, in bytes.
const MAX_NAME_LEN: usize = 100;

#[derive(Debug, Deserialize)]
struct CreateRequest {
    name: String,
    role: String,
}

/// One user, as listed. The token is never included.
#[derive(Debug, Serialize)]
struct UserRow {
    id: String,
    name: String,
    role: String,
    created_at: String,
}

/// `POST /v1/users`: creates a user and returns their token. The token is shown once.
pub async fn create(state: &AppState, body: &Bytes) -> Result<Response<Full<Bytes>>, ApiError> {
    let request: CreateRequest = serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    let name = request.name.trim().to_owned();
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(ApiError::BadRequest(format!(
            "name must be 1 to {MAX_NAME_LEN} characters"
        )));
    }
    let role = Role::parse(&request.role)
        .ok_or_else(|| ApiError::BadRequest("role must be admin, manager or driver".to_owned()))?;

    let id = format!("u_{}", random_hex(12)?);
    let token = format!("tpt_{}", random_hex(32)?);
    let hash = token_hash(&token);
    {
        let (id, name, hash) = (id.clone(), name.clone(), hash);
        state
            .db
            .run(move |connection| {
                connection.execute(
                    "INSERT INTO users (id, name, role, token_hash) VALUES (?1, ?2, ?3, ?4)",
                    params![id, name, role.as_str(), hash],
                )?;
                Ok(())
            })
            .await?;
    }

    Ok(json_response(
        StatusCode::CREATED,
        &json!({
            "id": id,
            "name": name,
            "role": role.as_str(),
            "token": token,
            "note": "Store this token now. It cannot be shown again.",
        }),
    ))
}

/// `GET /v1/users`: lists users, oldest first.
pub async fn list(state: &AppState) -> Result<Response<Full<Bytes>>, ApiError> {
    let users = state
        .db
        .run(|connection| {
            let mut statement = connection
                .prepare("SELECT id, name, role, created_at FROM users ORDER BY created_at, id")?;
            let rows = statement.query_map([], |row| {
                Ok(UserRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    role: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(json_response(StatusCode::OK, &json!({ "users": users })))
}

/// `DELETE /v1/users/{id}`: deletes a user and every trip they recorded.
pub async fn delete(state: &AppState, id: &str) -> Result<Response<Full<Bytes>>, ApiError> {
    let id = id.to_owned();
    let (user, trips) = {
        let user = id.clone();
        state
            .db
            .run(move |connection| {
                // Returning early drops the transaction, which rolls it back.
                let transaction = connection.transaction()?;
                let trips: i64 = transaction.query_row(
                    "SELECT COUNT(*) FROM trips WHERE user_id = ?1",
                    params![user],
                    |row| row.get(0),
                )?;
                let removed =
                    transaction.execute("DELETE FROM users WHERE id = ?1", params![user])?;
                if removed == 0 {
                    return Err(ApiError::NotFound);
                }
                transaction.commit()?;
                Ok((user, trips))
            })
            .await?
    };
    tracing::info!(user = %user, trips, "user deleted");
    Ok(json_response(
        StatusCode::OK,
        &json!({ "deleted_user": user, "deleted_trips": trips }),
    ))
}

/// `POST /v1/users/{id}/token`: issues the user a new token. The old token stops working at once.
pub async fn rotate_token(state: &AppState, id: &str) -> Result<Response<Full<Bytes>>, ApiError> {
    let token = format!("tpt_{}", random_hex(32)?);
    let hash = token_hash(&token);
    let id = id.to_owned();
    let changed = {
        let id = id.clone();
        state
            .db
            .run(move |connection| {
                Ok(connection.execute(
                    "UPDATE users SET token_hash = ?2 WHERE id = ?1",
                    params![id, hash],
                )?)
            })
            .await?
    };
    if changed == 0 {
        return Err(ApiError::NotFound);
    }
    tracing::info!(user = %id, "user token rotated");
    Ok(json_response(
        StatusCode::OK,
        &json!({
            "id": id,
            "token": token,
            "note": "Store this token now. It cannot be shown again, and the old token no longer works.",
        }),
    ))
}

#[derive(Debug, Deserialize)]
struct PasswordRequest {
    password: String,
}

/// `PUT /v1/users/{id}/password`: sets the user's password, which lets them sign in with `POST /v1/sessions`.
/// Every session the user holds ends at once, so an old sign-in cannot outlive a password change.
pub async fn set_password(
    state: &AppState,
    id: &str,
    body: &Bytes,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let request: PasswordRequest = serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    passwords::check_policy(&request.password)?;
    let stored = passwords::hash(&request.password)?;

    let id = id.to_owned();
    let ended = {
        let id = id.clone();
        state
            .db
            .run(move |connection| {
                let transaction = connection.transaction()?;
                let changed = transaction.execute(
                    "UPDATE users SET password_hash = ?2 WHERE id = ?1",
                    params![id, stored],
                )?;
                if changed == 0 {
                    return Err(ApiError::NotFound);
                }
                let ended =
                    transaction.execute("DELETE FROM sessions WHERE user_id = ?1", params![id])?;
                transaction.commit()?;
                Ok(ended)
            })
            .await?
    };
    tracing::info!(user = %id, sessions_ended = ended, "user password set");
    Ok(json_response(
        StatusCode::OK,
        &json!({ "id": id, "password_set": true, "sessions_ended": ended }),
    ))
}
