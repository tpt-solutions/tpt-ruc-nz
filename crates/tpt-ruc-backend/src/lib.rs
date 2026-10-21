// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Self-hostable backend for tpt-ruc-nz.
//!
//! The API is versioned under `/v1`. Each request is signed in with the
//! owner's API key or with a user's token, and the role decides what the
//! caller may do. Health endpoints are open so a container orchestrator can
//! probe them.
//!
//! The HTTP routing, CORS handling and request logging are written here rather
//! than taken from a web framework, so the dependency tree stays within the
//! project's licence policy. Hyper does the HTTP/1 protocol work.

pub mod alerts;
pub mod apikey;
pub mod auth;
pub mod clock;
pub mod config;
pub mod data;
pub mod db;
pub mod devices;
pub mod error;
pub mod eruc;
pub mod eruc_jobs;
pub mod fleet;
pub mod healthcheck;
pub mod pdf;
pub mod ratelimit;
pub mod reports;
pub mod server;
pub mod static_files;
pub mod trips;
pub mod users;
pub mod vehicles;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use http::header::{self, HeaderValue};
use http::{Method, Request, Response, StatusCode};
use http_body_util::Full;
use serde::Serialize;
use serde_json::json;

use crate::auth::{Principal, Role};
use crate::db::Database;
use crate::error::ApiError;
use crate::eruc::PushProvider;

/// Largest request body accepted. A full batch of trips is far smaller than this.
pub const MAX_BODY_BYTES: usize = 256 * 1024;

/// State shared by every request handler.
pub struct AppState {
    /// The database. Cloning is cheap, so the state can be shared behind an `Arc`.
    pub db: Database,
    /// The owner's API key.
    pub api_key: String,
    /// Browser origins allowed by CORS. Empty means no cross-origin access.
    pub cors_origins: Vec<String>,
    /// Request and failed-login limits, applied per client address.
    pub limiter: ratelimit::RateLimiter,
    /// Folder of the built web app to serve, from `TPT_RUC_PWA_DIR`. `None` serves no pages.
    pub pwa_dir: Option<PathBuf>,
    /// The provider eRUC jobs are pushed to, from `TPT_RUC_ERUC_PROVIDER`. `None` disables pushing.
    pub eruc_push: Option<Arc<dyn PushProvider>>,
}

/// Handles one request. The body has already been read, and is at most [`MAX_BODY_BYTES`].
pub async fn handle(state: &AppState, request: Request<Bytes>) -> Response<Full<Bytes>> {
    let started = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();

    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .filter(|origin| state.cors_origins.iter().any(|allowed| allowed == origin))
        .map(str::to_owned);

    let mut response = if method == Method::OPTIONS && origin.is_some() {
        preflight()
    } else {
        route(state, &request).await
    };

    if let Some(origin) = origin {
        if let Ok(value) = HeaderValue::from_str(&origin) {
            response
                .headers_mut()
                .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
        }
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Origin"));
    }

    tracing::info!(
        method = %method,
        path = %path,
        status = response.status().as_u16(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "request"
    );
    response
}

async fn route(state: &AppState, request: &Request<Bytes>) -> Response<Full<Bytes>> {
    let path = request.uri().path();
    let method = request.method();
    let query = request.uri().query();
    match path {
        "/healthz" => match method {
            &Method::GET => json_response(StatusCode::OK, &json!({ "status": "ok" })),
            _ => method_not_allowed(&[Method::GET]),
        },
        "/readyz" => match method {
            &Method::GET => ready(state).await,
            _ => method_not_allowed(&[Method::GET]),
        },
        "/v1/trips" => {
            let principal =
                match authorize(state, request, &[Method::GET, Method::POST, Method::DELETE]).await
                {
                    Ok(principal) => principal,
                    Err(response) => return response,
                };
            let result = if method == Method::POST {
                trips::ingest(state, &principal, request.headers(), request.body()).await
            } else if method == Method::DELETE {
                data::delete_trips(state, &principal, query).await
            } else {
                trips::list(state, &principal, query).await
            };
            result.unwrap_or_else(|error| error.into_response())
        }
        "/v1/devices" => {
            let principal = match authorize(state, request, &[Method::GET, Method::POST]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            let result = if method == Method::POST {
                devices::register(state, &principal, request.body()).await
            } else {
                devices::list(state, &principal).await
            };
            result.unwrap_or_else(|error| error.into_response())
        }
        _ if path.starts_with("/v1/devices/") => {
            let principal = match authorize(state, request, &[Method::DELETE]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            let id = &path["/v1/devices/".len()..];
            if id.is_empty() || id.contains('/') {
                return ApiError::NotFound.into_response();
            }
            devices::revoke(state, &principal, id)
                .await
                .unwrap_or_else(|error| error.into_response())
        }
        "/v1/export" => {
            let principal = match authorize(state, request, &[Method::GET]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            data::export(state, &principal)
                .await
                .unwrap_or_else(|error| error.into_response())
        }
        "/v1/reports/trips.csv" | "/v1/reports/monthly.csv" | "/v1/reports/monthly.pdf" => {
            let principal = match authorize(state, request, &[Method::GET]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            if let Err(error) = principal.require(Role::Manager) {
                return error.into_response();
            }
            let result = match path {
                "/v1/reports/trips.csv" => reports::trips_csv(state, query).await,
                "/v1/reports/monthly.pdf" => reports::monthly_pdf(state, query).await,
                _ => reports::monthly_csv(state, query).await,
            };
            result.unwrap_or_else(|error| error.into_response())
        }
        "/v1/eruc/export" => {
            let principal = match authorize(state, request, &[Method::GET]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            if let Err(error) = principal.require(Role::Manager) {
                return error.into_response();
            }
            eruc::export(state, query)
                .await
                .unwrap_or_else(|error| error.into_response())
        }
        "/v1/eruc/jobs" => {
            let principal = match authorize(state, request, &[Method::GET, Method::POST]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            if let Err(error) = principal.require(Role::Manager) {
                return error.into_response();
            }
            let result = if method == Method::POST {
                eruc_jobs::create(state, &principal, request.body()).await
            } else {
                eruc_jobs::list(state).await
            };
            result.unwrap_or_else(|error| error.into_response())
        }
        _ if path.starts_with("/v1/eruc/jobs/") => {
            let principal = match authorize(state, request, &[Method::GET]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            if let Err(error) = principal.require(Role::Manager) {
                return error.into_response();
            }
            eruc_jobs::get(state, &path["/v1/eruc/jobs/".len()..])
                .await
                .unwrap_or_else(|error| error.into_response())
        }
        "/v1/fleet/summary" | "/v1/alerts" => {
            let principal = match authorize(state, request, &[Method::GET]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            if let Err(error) = principal.require(Role::Manager) {
                return error.into_response();
            }
            let result = if path == "/v1/alerts" {
                alerts::list(state, query).await
            } else {
                fleet::summary(state, query).await
            };
            result.unwrap_or_else(|error| error.into_response())
        }
        "/v1/users" => {
            let principal = match authorize(state, request, &[Method::GET, Method::POST]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            if let Err(error) = principal.require(Role::Admin) {
                return error.into_response();
            }
            let result = if method == Method::POST {
                users::create(state, request.body()).await
            } else {
                users::list(state).await
            };
            result.unwrap_or_else(|error| error.into_response())
        }
        _ if path.starts_with("/v1/users/") => {
            let rest = &path["/v1/users/".len()..];
            let (allowed, token_for) = match rest.strip_suffix("/token") {
                Some(id) => (Method::POST, Some(id)),
                None => (Method::DELETE, None),
            };
            let principal = match authorize(state, request, &[allowed]).await {
                Ok(principal) => principal,
                Err(response) => return response,
            };
            if let Err(error) = principal.require(Role::Admin) {
                return error.into_response();
            }
            let result = match token_for {
                Some(id) if !id.is_empty() && !id.contains('/') => {
                    users::rotate_token(state, id).await
                }
                Some(_) => Err(ApiError::NotFound),
                None if !rest.is_empty() && !rest.contains('/') => users::delete(state, rest).await,
                None => Err(ApiError::NotFound),
            };
            result.unwrap_or_else(|error| error.into_response())
        }
        _ if path == "/v1/vehicles" || path.starts_with("/v1/vehicles/") => {
            vehicles::route(state, request).await
        }
        _ => match &state.pwa_dir {
            Some(root)
                if !path.starts_with("/v1/")
                    && (method == Method::GET || method == Method::HEAD) =>
            {
                static_files::serve(root, path, method == Method::HEAD).await
            }
            _ => json_response(StatusCode::NOT_FOUND, &json!({ "error": "not found" })),
        },
    }
}

/// Checks the method, then works out who is calling. Returns the response to
/// send when the request must stop here.
pub(crate) async fn authorize(
    state: &AppState,
    request: &Request<Bytes>,
    allowed: &[Method],
) -> Result<Principal, Response<Full<Bytes>>> {
    if !allowed.contains(request.method()) {
        return Err(method_not_allowed(allowed));
    }
    auth::resolve(state, request.headers())
        .await
        .map_err(ApiError::into_response)
}

/// Readiness: the database answers a trivial query.
async fn ready(state: &AppState) -> Response<Full<Bytes>> {
    match state.db.ping().await {
        Ok(()) => json_response(StatusCode::OK, &json!({ "status": "ready" })),
        Err(_) => json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &json!({ "status": "unavailable" }),
        ),
    }
}

/// Answers a CORS preflight for an allowed origin.
fn preflight() -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::new()));
    *response.status_mut() = StatusCode::NO_CONTENT;
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, DELETE"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("authorization, content-type"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    response
}

fn method_not_allowed(allowed: &[Method]) -> Response<Full<Bytes>> {
    let allow = allowed
        .iter()
        .map(Method::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let mut response = json_response(
        StatusCode::METHOD_NOT_ALLOWED,
        &json!({ "error": "method not allowed" }),
    );
    if let Ok(value) = HeaderValue::from_str(&allow) {
        response.headers_mut().insert(header::ALLOW, value);
    }
    response
}

/// Returns the value of one query parameter, such as `from` in `?from=2026-01-01`.
/// The first occurrence wins. Absent means `None`.
pub fn query_value<'a>(query: Option<&'a str>, name: &str) -> Option<&'a str> {
    query?.split('&').find_map(|pair| {
        pair.strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
    })
}

/// Builds a JSON response with the given status.
pub fn json_response<T: Serialize + ?Sized>(status: StatusCode, body: &T) -> Response<Full<Bytes>> {
    let bytes = match serde_json::to_vec(body) {
        Ok(bytes) => bytes,
        Err(_) => br#"{"error":"internal error"}"#.to_vec(),
    };
    let mut response = Response::new(Full::new(Bytes::from(bytes)));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}
