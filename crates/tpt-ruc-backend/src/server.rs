// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The HTTP/1 accept loop. Hyper does the protocol work; this module owns the
//! connection limits, the body-size check and rate limiting.

use std::convert::Infallible;
use std::future::Future;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http::header::{self, HeaderValue};
use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use serde_json::json;
use tokio::net::TcpListener;

use crate::{handle, json_response, AppState, MAX_BODY_BYTES};

/// Longest a client may take to send its request headers. This stops slow-header attacks.
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Serves requests until `shutdown` completes. Connections already in progress are
/// abandoned, which is acceptable for a single-owner self-hosted server.
pub async fn serve(
    listener: TcpListener,
    state: Arc<AppState>,
    shutdown: impl Future<Output = ()>,
) -> std::io::Result<()> {
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => return Ok(()),
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(pair) => pair,
                    Err(error) => {
                        // Accept errors such as running out of file descriptors are usually
                        // transient, so log them and keep serving.
                        tracing::warn!(%error, "accept failed");
                        continue;
                    }
                };
                let state = Arc::clone(&state);
                let client = peer.ip();
                tokio::spawn(async move {
                    let service = service_fn(move |request| {
                        let state = Arc::clone(&state);
                        async move { Ok::<_, Infallible>(serve_one(&state, client, request).await) }
                    });
                    let result = http1::Builder::new()
                        .timer(TokioTimer::new())
                        .header_read_timeout(HEADER_READ_TIMEOUT)
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                    if let Err(error) = result {
                        tracing::debug!(%error, "connection closed with error");
                    }
                });
            }
        }
    }
}

/// Applies rate limits, reads the body with its size limit, then hands the request to the router.
async fn serve_one(
    state: &AppState,
    client: IpAddr,
    request: Request<Incoming>,
) -> Response<Full<Bytes>> {
    if let Err(wait) = state.limiter.check_request(client, Instant::now()) {
        return too_many_requests(wait);
    }

    let (parts, body) = request.into_parts();
    let response = match Limited::new(body, MAX_BODY_BYTES).collect().await {
        Ok(collected) => handle(state, Request::from_parts(parts, collected.to_bytes())).await,
        Err(error) if error.downcast_ref::<LengthLimitError>().is_some() => json_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            &json!({ "error": "request body too large" }),
        ),
        Err(_) => json_response(
            StatusCode::BAD_REQUEST,
            &json!({ "error": "request body could not be read" }),
        ),
    };

    if response.status() == StatusCode::UNAUTHORIZED {
        state.limiter.record_auth_failure(client, Instant::now());
    }
    response
}

/// The response for a refused request, with the number of seconds to wait.
fn too_many_requests(wait: Duration) -> Response<Full<Bytes>> {
    let mut response = json_response(
        StatusCode::TOO_MANY_REQUESTS,
        &json!({ "error": "too many requests" }),
    );
    let seconds = wait.as_secs().max(1);
    if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}
