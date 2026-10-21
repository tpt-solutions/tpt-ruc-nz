// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! API errors and their JSON bodies.

use bytes::Bytes;
use http::header::{self, HeaderValue};
use http::{Response, StatusCode};
use http_body_util::Full;
use serde_json::json;

use crate::json_response;
use crate::trips::ItemResult;

/// An error returned to the client.
#[derive(Debug)]
pub enum ApiError {
    /// The API key is missing or wrong.
    Unauthorized,
    /// The request is malformed as a whole.
    BadRequest(String),
    /// One or more trips failed validation. Nothing in the batch was stored.
    Validation(Vec<ItemResult>),
    /// A trip ID was already stored with different values. Nothing in the batch was stored.
    Conflict(Vec<ItemResult>),
    /// The caller is signed in but may not do this.
    Forbidden,
    /// The item named in the request does not exist.
    NotFound,
    /// A device upload reused a sequence number, or sent one older than the last accepted.
    Replay,
    /// Something failed on the server. The detail is logged, not returned.
    Internal,
}

impl ApiError {
    /// Converts the error into the HTTP response sent to the client.
    pub fn into_response(self) -> Response<Full<Bytes>> {
        match self {
            Self::Unauthorized => {
                let mut response = json_response(
                    StatusCode::UNAUTHORIZED,
                    &json!({ "error": "unauthorized" }),
                );
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
                response
            }
            Self::BadRequest(message) => {
                json_response(StatusCode::BAD_REQUEST, &json!({ "error": message }))
            }
            Self::Validation(results) => json_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                &json!({ "error": "validation failed", "results": results }),
            ),
            Self::Conflict(results) => json_response(
                StatusCode::CONFLICT,
                &json!({ "error": "conflict", "results": results }),
            ),
            Self::Forbidden => {
                json_response(StatusCode::FORBIDDEN, &json!({ "error": "forbidden" }))
            }
            Self::NotFound => {
                json_response(StatusCode::NOT_FOUND, &json!({ "error": "not found" }))
            }
            Self::Replay => json_response(
                StatusCode::CONFLICT,
                &json!({ "error": "sequence already used or older than the last upload from this device" }),
            ),
            Self::Internal => json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &json!({ "error": "internal error" }),
            ),
        }
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> Self {
        tracing::error!(%error, "database error");
        Self::Internal
    }
}
