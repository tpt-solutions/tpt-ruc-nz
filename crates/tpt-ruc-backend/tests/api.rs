// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! End-to-end tests of the HTTP API, run against an in-memory database.

use std::sync::Arc;

use bytes::Bytes;
use http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tpt_ruc_backend::db::Database;
use tpt_ruc_backend::ratelimit::RateLimiter;
use tpt_ruc_backend::reports::nzd;
use tpt_ruc_backend::server::serve;
use tpt_ruc_backend::{handle, AppState};
use tpt_ruc_core::ruc::{liability, RateDate, Vehicle};
use tpt_ruc_core::ruc_rates::NZTA_SCHEDULE;

const KEY: &str = "0123456789abcdef0123456789abcdef";
const ORIGIN: &str = "https://app.example";

fn state() -> AppState {
    AppState {
        db: Database::open(":memory:").unwrap(),
        api_key: KEY.to_owned(),
        cors_origins: vec![ORIGIN.to_owned()],
        limiter: RateLimiter::default(),
    }
}

/// Sends one request through the router and returns the status, headers and JSON body.
async fn send_full(
    state: &AppState,
    method: Method,
    uri: &str,
    body: Option<Value>,
    authed: bool,
) -> (StatusCode, http::HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if authed {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {KEY}"));
    }
    let body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Bytes::from(value.to_string())
        }
        None => Bytes::new(),
    };
    let response = handle(state, builder.body(body).unwrap()).await;
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, value)
}

async fn send(
    state: &AppState,
    method: Method,
    uri: &str,
    body: Option<Value>,
    authed: bool,
) -> (StatusCode, Value) {
    let (status, _, value) = send_full(state, method, uri, body, authed).await;
    (status, value)
}

fn trip(id: &str, date: &str, metres: f64) -> Value {
    json!({
        "id": id,
        "date": date,
        "metres": metres,
        "vehicle": { "rate_type": "1", "gross_kg": 3000, "electric": false }
    })
}

/// The charge the core calculates for the same trip, so tests need no hardcoded amounts.
fn expected_incl_gst(date: RateDate, metres: f64) -> u64 {
    let vehicle = Vehicle {
        rate_type: "1",
        gross_kg: 3000,
        electric: false,
    };
    liability(NZTA_SCHEDULE, &vehicle, date, metres)
        .unwrap()
        .incl_gst_micros
}

const TRIP_DATE: RateDate = RateDate {
    year: 2026,
    month: 10,
    day: 1,
};

#[tokio::test]
async fn health_endpoints_need_no_key() {
    let state = state();
    let (status, _) = send(&state, Method::GET, "/healthz", None, false).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&state, Method::GET, "/readyz", None, false).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn api_refuses_missing_or_wrong_key() {
    let state = state();
    let (status, headers, body) = send_full(&state, Method::GET, "/v1/trips", None, false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "unauthorized");
    assert_eq!(headers[header::WWW_AUTHENTICATE], "Bearer");

    let request = Request::builder()
        .uri("/v1/trips")
        .header(header::AUTHORIZATION, "Bearer wrong-key")
        .body(Bytes::new())
        .unwrap();
    assert_eq!(
        handle(&state, request).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn unknown_paths_and_wrong_methods_are_reported() {
    let state = state();
    let (status, _) = send(&state, Method::GET, "/nope", None, false).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, headers, _) = send_full(&state, Method::PUT, "/v1/trips", None, true).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(headers[header::ALLOW], "GET, POST, DELETE");
}

#[tokio::test]
async fn ingest_stores_a_trip_with_the_server_charge() {
    let state = state();
    let (status, body) = send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [trip("t-1", "2026-10-01", 12_345.0)] })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["results"][0]["status"], "created");
    assert_eq!(
        body["results"][0]["liability"]["incl_gst_micros"],
        expected_incl_gst(TRIP_DATE, 12_345.0)
    );

    let (status, list) = send(&state, Method::GET, "/v1/trips", None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["trips"].as_array().unwrap().len(), 1);
    assert_eq!(list["trips"][0]["id"], "t-1");
    assert_eq!(list["trips"][0]["billed_metres"], 12_345);
    assert_eq!(list["trips"][0]["electric"], false);
}

#[tokio::test]
async fn client_supplied_charges_are_ignored() {
    let state = state();
    let mut tampered = trip("t-2", "2026-10-01", 1_000.0);
    tampered["incl_gst_micros"] = json!(1);
    tampered["liability"] = json!({ "incl_gst_micros": 1 });
    let (status, body) = send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [tampered] })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["results"][0]["liability"]["incl_gst_micros"],
        expected_incl_gst(TRIP_DATE, 1_000.0)
    );
}

#[tokio::test]
async fn resending_the_same_trip_is_unchanged() {
    let state = state();
    let payload = json!({ "trips": [trip("t-3", "2026-10-01", 500.0)] });
    send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(payload.clone()),
        true,
    )
    .await;

    let (status, body) = send(&state, Method::POST, "/v1/trips", Some(payload), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["results"][0]["status"], "unchanged");

    let (_, list) = send(&state, Method::GET, "/v1/trips", None, true).await;
    assert_eq!(list["trips"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn changed_values_for_a_stored_id_conflict_and_store_nothing() {
    let state = state();
    send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [trip("t-4", "2026-10-01", 500.0)] })),
        true,
    )
    .await;

    let batch = json!({ "trips": [
        trip("t-5", "2026-10-02", 700.0),
        trip("t-4", "2026-10-01", 900.0)
    ] });
    let (status, body) = send(&state, Method::POST, "/v1/trips", Some(batch), true).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["results"][0]["status"], "not_stored");
    assert_eq!(body["results"][1]["status"], "conflict");

    let (_, list) = send(&state, Method::GET, "/v1/trips", None, true).await;
    assert_eq!(
        list["trips"].as_array().unwrap().len(),
        1,
        "t-5 must not be stored"
    );
}

#[tokio::test]
async fn a_batch_with_one_invalid_trip_stores_nothing() {
    let state = state();
    let mut bad = trip("t-6", "2026-10-01", 100.0);
    bad["vehicle"]["rate_type"] = json!("no-such-code");
    let batch = json!({ "trips": [trip("t-7", "2026-10-01", 100.0), bad] });

    let (status, body) = send(&state, Method::POST, "/v1/trips", Some(batch), true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["results"][0]["status"], "not_stored");
    assert_eq!(body["results"][1]["status"], "rejected");
    assert!(body["results"][1]["error"]
        .as_str()
        .unwrap()
        .contains("no-such-code"));

    let (_, list) = send(&state, Method::GET, "/v1/trips", None, true).await;
    assert_eq!(list["trips"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn a_date_with_no_rate_table_is_rejected() {
    let state = state();
    let (status, body) = send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [trip("t-8", "2020-01-01", 100.0)] })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("no RUC rates"));
}

#[tokio::test]
async fn malformed_and_empty_batches_are_bad_requests() {
    let state = state();
    let (status, _) = send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [] })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "nope": 1 })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let many: Vec<Value> = (0..501)
        .map(|i| trip(&format!("t-{i}"), "2026-10-01", 1.0))
        .collect();
    let (status, _) = send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": many })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_is_ordered_by_date_and_paged() {
    let state = state();
    send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [
            trip("b", "2026-10-03", 1.0),
            trip("a", "2026-10-01", 1.0),
            trip("c", "2026-10-02", 1.0)
        ] })),
        true,
    )
    .await;

    let (_, page) = send(
        &state,
        Method::GET,
        "/v1/trips?limit=2&offset=1",
        None,
        true,
    )
    .await;
    let ids: Vec<&str> = page["trips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["c", "b"]);
    assert_eq!(page["limit"], 2);
    assert_eq!(page["offset"], 1);

    let (status, _) = send(&state, Method::GET, "/v1/trips?limit=many", None, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cors_allows_only_configured_origins() {
    let state = state();
    let request = Request::builder()
        .uri("/healthz")
        .header(header::ORIGIN, ORIGIN)
        .body(Bytes::new())
        .unwrap();
    let response = handle(&state, request).await;
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        ORIGIN
    );

    let request = Request::builder()
        .uri("/healthz")
        .header(header::ORIGIN, "https://evil.example")
        .body(Bytes::new())
        .unwrap();
    let response = handle(&state, request).await;
    assert!(response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());

    let preflight = Request::builder()
        .method(Method::OPTIONS)
        .uri("/v1/trips")
        .header(header::ORIGIN, ORIGIN)
        .body(Bytes::new())
        .unwrap();
    let response = handle(&state, preflight).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        ORIGIN
    );
}

/// Starts the real server on an ephemeral port and returns its address.
async fn start_server() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let state = Arc::new(state());
    tokio::spawn(async move {
        serve(listener, state, std::future::pending::<()>())
            .await
            .unwrap();
    });
    address
}

/// Sends raw HTTP/1.1 bytes and returns the full response text.
async fn raw_request(address: std::net::SocketAddr, request: &[u8]) -> String {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(request).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8_lossy(&response).into_owned()
}

#[tokio::test]
async fn server_answers_health_over_http() {
    let address = start_server().await;
    let response = raw_request(
        address,
        b"GET /healthz HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains(r#"{"status":"ok"}"#), "{response}");
}

#[tokio::test]
async fn server_refuses_oversized_bodies_before_routing() {
    let address = start_server().await;
    let body = "x".repeat(300 * 1024);
    let request = format!(
        "POST /v1/trips HTTP/1.1\r\nHost: test\r\nAuthorization: Bearer {KEY}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let response = raw_request(address, request.as_bytes()).await;
    assert!(
        response.starts_with("HTTP/1.1 413"),
        "{}",
        &response[..response.len().min(200)]
    );
}

/// Sends a GET and returns the status, the content type and the body text.
async fn get_text(state: &AppState, uri: &str, authed: bool) -> (StatusCode, String, String) {
    let mut builder = Request::builder().method(Method::GET).uri(uri);
    if authed {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {KEY}"));
    }
    let response = handle(state, builder.body(Bytes::new()).unwrap()).await;
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        content_type,
        String::from_utf8(bytes.to_vec()).unwrap(),
    )
}

async fn ingest_trips(state: &AppState, trips: Vec<Value>) {
    let (status, body) = send(
        state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": trips })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn trips_report_lists_each_trip_with_its_charge() {
    let state = state();
    ingest_trips(
        &state,
        vec![
            trip("r-1", "2026-10-01", 12_345.0),
            trip("r-2", "2026-10-02", 1_000.0),
        ],
    )
    .await;

    let (status, content_type, csv) = get_text(&state, "/v1/reports/trips.csv", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "text/csv; charset=utf-8");
    let lines: Vec<&str> = csv.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3, "{csv}");
    assert!(lines[0].starts_with("trip_id,trip_date,vehicle_type"));

    let expected = nzd(expected_incl_gst(TRIP_DATE, 12_345.0) as i64);
    assert!(lines[1].starts_with("r-1,2026-10-01,1,3000,false,12345.000,12.345,"));
    assert!(lines[1].ends_with(&format!(",{expected}")), "{}", lines[1]);
}

#[tokio::test]
async fn trips_report_honours_the_date_range() {
    let state = state();
    ingest_trips(
        &state,
        vec![
            trip("early", "2026-09-30", 1.0),
            trip("inside", "2026-10-01", 1.0),
            trip("late", "2026-11-01", 1.0),
        ],
    )
    .await;

    let (_, _, csv) = get_text(
        &state,
        "/v1/reports/trips.csv?from=2026-10-01&to=2026-10-31",
        true,
    )
    .await;
    assert!(csv.contains("inside,"));
    assert!(!csv.contains("early,") && !csv.contains("late,"), "{csv}");
}

#[tokio::test]
async fn monthly_report_totals_each_month_and_vehicle_type() {
    let state = state();
    ingest_trips(
        &state,
        vec![
            trip("m-1", "2026-09-15", 1_000.0),
            trip("m-2", "2026-10-01", 2_000.0),
            trip("m-3", "2026-10-20", 3_000.0),
        ],
    )
    .await;

    let (status, _, csv) = get_text(&state, "/v1/reports/monthly.csv", true).await;
    assert_eq!(status, StatusCode::OK);
    let lines: Vec<&str> = csv.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3, "{csv}");
    assert!(lines[1].starts_with("2026-09,1,1,1.000,"), "{}", lines[1]);
    assert!(lines[2].starts_with("2026-10,1,2,5.000,"), "{}", lines[2]);

    let october = expected_incl_gst(TRIP_DATE, 2_000.0) + expected_incl_gst(TRIP_DATE, 3_000.0);
    assert!(
        lines[2].ends_with(&format!(",{}", nzd(october as i64))),
        "{}",
        lines[2]
    );
}

#[tokio::test]
async fn reports_need_the_key_and_valid_dates() {
    let state = state();
    let (status, _, _) = get_text(&state, "/v1/reports/trips.csv", false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = get_text(&state, "/v1/reports/monthly.csv?from=2026-13-01", true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn hostile_trip_ids_cannot_inject_formulas_into_reports() {
    let state = state();
    ingest_trips(&state, vec![trip("=HYPERLINK(\"x\")", "2026-10-01", 1.0)]).await;

    let (_, _, csv) = get_text(&state, "/v1/reports/trips.csv", true).await;
    assert!(csv.contains("\"'=HYPERLINK(\"\"x\"\")\""), "{csv}");
}

#[tokio::test]
async fn export_returns_every_trip_with_its_stored_charge() {
    let state = state();
    ingest_trips(
        &state,
        vec![
            trip("e-1", "2026-09-01", 1_000.0),
            trip("e-2", "2026-10-01", 2_000.0),
        ],
    )
    .await;

    let (status, body) = send(&state, Method::GET, "/v1/export", None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["format"], "tpt-ruc-export");
    assert_eq!(body["version"], 1);
    let trips = body["trips"].as_array().unwrap();
    assert_eq!(trips.len(), 2);
    assert_eq!(trips[1]["id"], "e-2");
    assert_eq!(
        trips[1]["incl_gst_micros"],
        expected_incl_gst(TRIP_DATE, 2_000.0)
    );
}

#[tokio::test]
async fn delete_removes_only_trips_in_the_inclusive_range() {
    let state = state();
    ingest_trips(
        &state,
        vec![
            trip("d-1", "2026-09-30", 1.0),
            trip("d-2", "2026-10-01", 1.0),
            trip("d-3", "2026-10-31", 1.0),
            trip("d-4", "2026-11-01", 1.0),
        ],
    )
    .await;

    let (status, body) = send(
        &state,
        Method::DELETE,
        "/v1/trips?from=2026-10-01&to=2026-10-31",
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["deleted"], 2);

    let (_, export) = send(&state, Method::GET, "/v1/export", None, true).await;
    let ids: Vec<&str> = export["trips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["d-1", "d-4"]);
}

#[tokio::test]
async fn delete_needs_both_dates_and_a_valid_range() {
    let state = state();
    let (status, _) = send(&state, Method::DELETE, "/v1/trips", None, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &state,
        Method::DELETE,
        "/v1/trips?from=2026-10-01",
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &state,
        Method::DELETE,
        "/v1/trips?from=2026-10-02&to=2026-10-01",
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_wide_range_deletes_everything() {
    let state = state();
    ingest_trips(&state, vec![trip("w-1", "2026-10-01", 1.0)]).await;

    let (_, body) = send(
        &state,
        Method::DELETE,
        "/v1/trips?from=0000-01-01&to=9999-12-31",
        None,
        true,
    )
    .await;
    assert_eq!(body["deleted"], 1);
    let (_, export) = send(&state, Method::GET, "/v1/export", None, true).await;
    assert_eq!(export["trips"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn export_and_delete_need_the_key() {
    let state = state();
    let (status, _) = send(&state, Method::GET, "/v1/export", None, false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(
        &state,
        Method::DELETE,
        "/v1/trips?from=0000-01-01&to=9999-12-31",
        None,
        false,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn repeated_bad_keys_block_the_client_with_retry_after() {
    let address = start_server().await;
    for _ in 0..10 {
        let response = raw_request(
            address,
            b"GET /v1/trips HTTP/1.1
Host: test
Authorization: Bearer wrong
Connection: close

",
        )
        .await;
        assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    }
    let request = format!(
        "GET /v1/trips HTTP/1.1
Host: test
Authorization: Bearer {KEY}
Connection: close

"
    );
    let response = raw_request(address, request.as_bytes()).await;
    assert!(response.starts_with("HTTP/1.1 429"), "{response}");
    assert!(
        response.to_lowercase().contains("retry-after: "),
        "{response}"
    );
}

/// Sends one request with a specific bearer token.
async fn send_as(
    state: &AppState,
    token: &str,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    let body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Bytes::from(value.to_string())
        }
        None => Bytes::new(),
    };
    let response = handle(state, builder.body(body).unwrap()).await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Creates a user as the owner and returns the user's ID and token.
async fn create_user(state: &AppState, name: &str, role: &str) -> (String, String) {
    let (status, body) = send(
        state,
        Method::POST,
        "/v1/users",
        Some(json!({ "name": name, "role": role })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        body["id"].as_str().unwrap().to_owned(),
        body["token"].as_str().unwrap().to_owned(),
    )
}

/// Registers a vehicle as the owner and assigns it to the given drivers. Returns its ID.
async fn assigned_vehicle(state: &AppState, driver_ids: &[&str]) -> String {
    let (status, vehicle) = send(
        state,
        Method::POST,
        "/v1/vehicles",
        Some(json!({ "name": "Van", "rate_type": "1", "gross_kg": 3000 })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{vehicle}");
    let id = vehicle["id"].as_str().unwrap().to_owned();
    let (status, body) = send(
        state,
        Method::PUT,
        &format!("/v1/vehicles/{id}/drivers"),
        Some(json!({ "user_ids": driver_ids })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    id
}

/// Points a trip at a registered vehicle instead of giving its fields inline.
fn on_vehicle(mut trip: Value, vehicle_id: &str) -> Value {
    trip.as_object_mut().unwrap().remove("vehicle");
    trip["vehicle_id"] = json!(vehicle_id);
    trip
}

fn ids_in(list: &Value, key: &str) -> Vec<String> {
    list[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn user_tokens_are_shown_once_and_never_listed() {
    let state = state();
    let (_, token) = create_user(&state, "Aroha", "driver").await;
    assert!(token.starts_with("tpt_"));

    let (status, list) = send(&state, Method::GET, "/v1/users", None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["users"][0]["name"], "Aroha");
    assert!(list["users"][0].get("token").is_none());
    assert!(!list.to_string().contains(&token));
}

#[tokio::test]
async fn an_unknown_token_is_refused() {
    let state = state();
    create_user(&state, "Aroha", "driver").await;
    let (status, _) = send_as(
        &state,
        "tpt_not-a-real-token",
        Method::GET,
        "/v1/trips",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn drivers_see_only_their_own_trips_and_managers_see_all() {
    let state = state();
    let (aroha_id, aroha) = create_user(&state, "Aroha", "driver").await;
    let (ben_id, ben) = create_user(&state, "Ben", "driver").await;
    let aroha_van = assigned_vehicle(&state, &[&aroha_id]).await;
    let ben_van = assigned_vehicle(&state, &[&ben_id]).await;
    let (_, mira) = create_user(&state, "Mira", "manager").await;

    let (status, _) = send_as(
        &state,
        &aroha,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [on_vehicle(trip("aroha-1", "2026-10-01", 1_000.0), &aroha_van)] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_as(
        &state,
        &ben,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [on_vehicle(trip("ben-1", "2026-10-01", 1_000.0), &ben_van)] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, own) = send_as(&state, &aroha, Method::GET, "/v1/trips", None).await;
    assert_eq!(ids_in(&own, "trips"), vec!["aroha-1"]);

    let (_, export) = send_as(&state, &ben, Method::GET, "/v1/export", None).await;
    assert_eq!(ids_in(&export, "trips"), vec!["ben-1"]);

    let (_, all) = send_as(&state, &mira, Method::GET, "/v1/trips", None).await;
    assert_eq!(ids_in(&all, "trips"), vec!["aroha-1", "ben-1"]);
    let owner_all = send(&state, Method::GET, "/v1/trips", None, true).await.1;
    assert_eq!(ids_in(&owner_all, "trips").len(), 2);
}

#[tokio::test]
async fn trip_ids_are_private_to_each_driver() {
    let state = state();
    let (aroha_id, aroha) = create_user(&state, "Aroha", "driver").await;
    let (ben_id, ben) = create_user(&state, "Ben", "driver").await;
    let van = assigned_vehicle(&state, &[&aroha_id, &ben_id]).await;
    send_as(
        &state,
        &aroha,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [on_vehicle(trip("shared", "2026-10-01", 1_000.0), &van)] })),
    )
    .await;

    // Ben reuses the ID for his own trip. He gets a new trip, and learns nothing about Aroha's.
    let (status, body) = send_as(
        &state,
        &ben,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [on_vehicle(trip("shared", "2026-10-02", 2_000.0), &van)] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["results"][0]["status"], "created");

    // Deleting everything Ben can see removes only his own trip.
    let (_, deleted) = send_as(
        &state,
        &ben,
        Method::DELETE,
        "/v1/trips?from=0000-01-01&to=9999-12-31",
        None,
    )
    .await;
    assert_eq!(deleted["deleted"], 1);
    let (_, own) = send_as(&state, &aroha, Method::GET, "/v1/trips", None).await;
    assert_eq!(ids_in(&own, "trips"), vec!["shared"]);
    assert_eq!(own["trips"][0]["trip_date"], "2026-10-01");
}

#[tokio::test]
async fn only_managers_run_reports_and_only_admins_manage_users() {
    let state = state();
    let (_, driver) = create_user(&state, "Aroha", "driver").await;
    let (_, manager) = create_user(&state, "Mira", "manager").await;
    let (_, admin) = create_user(&state, "Sam", "admin").await;

    let (status, _) = send_as(
        &state,
        &driver,
        Method::GET,
        "/v1/reports/monthly.csv",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send_as(
        &state,
        &manager,
        Method::GET,
        "/v1/reports/monthly.csv",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send_as(&state, &manager, Method::GET, "/v1/users", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send_as(&state, &admin, Method::GET, "/v1/users", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_as(
        &state,
        &driver,
        Method::POST,
        "/v1/users",
        Some(json!({ "name": "Eve", "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn deleting_a_user_removes_their_trips_and_unknown_ids_are_not_found() {
    let state = state();
    let (id, driver) = create_user(&state, "Aroha", "driver").await;
    let van = assigned_vehicle(&state, &[&id]).await;
    send_as(
        &state,
        &driver,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [on_vehicle(trip("gone-1", "2026-10-01", 1.0), &van), on_vehicle(trip("gone-2", "2026-10-02", 1.0), &van)] })),
    )
    .await;

    let (status, body) = send(
        &state,
        Method::DELETE,
        &format!("/v1/users/{id}"),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["deleted_trips"], 2);

    let (_, export) = send(&state, Method::GET, "/v1/export", None, true).await;
    assert_eq!(export["trips"].as_array().unwrap().len(), 0);
    let (status, _) = send_as(&state, &driver, Method::GET, "/v1/trips", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &state,
        Method::DELETE,
        &format!("/v1/users/{id}"),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn owner_trips_have_no_user_and_bad_roles_are_refused() {
    let state = state();
    let (status, body) = send(
        &state,
        Method::POST,
        "/v1/users",
        Some(json!({ "name": "Eve", "role": "owner" })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [trip("owner-1", "2026-10-01", 1.0)] })),
        true,
    )
    .await;
    let (_, export) = send(&state, Method::GET, "/v1/export", None, true).await;
    assert!(export["trips"][0]["user_id"].is_null());
}

#[tokio::test]
async fn drivers_may_only_record_trips_on_vehicles_assigned_to_them() {
    let state = state();
    let (aroha_id, aroha) = create_user(&state, "Aroha", "driver").await;
    let (ben_id, _) = create_user(&state, "Ben", "driver").await;
    let aroha_van = assigned_vehicle(&state, &[&aroha_id]).await;
    let ben_van = assigned_vehicle(&state, &[&ben_id]).await;

    // An inline vehicle is refused for a driver.
    let (status, body) = send_as(
        &state,
        &aroha,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [trip("inline", "2026-10-01", 1.0)] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(body["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("assigned to them"));

    // A vehicle assigned to someone else is refused as not found.
    let (status, body) = send_as(
        &state,
        &aroha,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [on_vehicle(trip("theirs", "2026-10-01", 1.0), &ben_van)] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(body["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("not found or not assigned"));

    // The assigned vehicle works, and its registered fields set the charge.
    let (status, body) = send_as(
        &state,
        &aroha,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [on_vehicle(trip("mine", "2026-10-01", 1_000.0), &aroha_van)] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["results"][0]["liability"]["incl_gst_micros"],
        expected_incl_gst(TRIP_DATE, 1_000.0)
    );
}

#[tokio::test]
async fn drivers_see_only_their_assigned_vehicles() {
    let state = state();
    let (aroha_id, aroha) = create_user(&state, "Aroha", "driver").await;
    let (ben_id, _) = create_user(&state, "Ben", "driver").await;
    let aroha_van = assigned_vehicle(&state, &[&aroha_id]).await;
    let ben_van = assigned_vehicle(&state, &[&ben_id]).await;

    let (_, list) = send_as(&state, &aroha, Method::GET, "/v1/vehicles", None).await;
    assert_eq!(ids_in(&list, "vehicles"), vec![aroha_van.clone()]);

    let (status, _) = send_as(
        &state,
        &aroha,
        Method::GET,
        &format!("/v1/vehicles/{ben_van}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send_as(
        &state,
        &aroha,
        Method::GET,
        &format!("/v1/vehicles/{aroha_van}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn only_managers_change_vehicles() {
    let state = state();
    let (_, driver) = create_user(&state, "Aroha", "driver").await;
    let (_, manager) = create_user(&state, "Mira", "manager").await;
    let body = json!({ "name": "Truck", "rate_type": "1", "gross_kg": 3000 });

    let (status, _) = send_as(
        &state,
        &driver,
        Method::POST,
        "/v1/vehicles",
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, created) =
        send_as(&state, &manager, Method::POST, "/v1/vehicles", Some(body)).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_owned();

    let (status, _) = send_as(
        &state,
        &driver,
        Method::DELETE,
        &format!("/v1/vehicles/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send_as(
        &state,
        &manager,
        Method::DELETE,
        &format!("/v1/vehicles/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn vehicle_input_is_validated() {
    let state = state();
    let (status, _) = send(
        &state,
        Method::POST,
        "/v1/vehicles",
        Some(json!({ "name": "X", "rate_type": "no-such-code", "gross_kg": 3000 })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let id = assigned_vehicle(&state, &[]).await;
    let (status, _) = send(
        &state,
        Method::PUT,
        &format!("/v1/vehicles/{id}/drivers"),
        Some(json!({ "user_ids": ["u_nobody"] })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deleting_a_vehicle_keeps_its_trips() {
    let state = state();
    let vehicle = assigned_vehicle(&state, &[]).await;
    send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [on_vehicle(trip("kept", "2026-10-01", 1.0), &vehicle)] })),
        true,
    )
    .await;
    let (status, _) = send(
        &state,
        Method::DELETE,
        &format!("/v1/vehicles/{vehicle}"),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, export) = send(&state, Method::GET, "/v1/export", None, true).await;
    assert_eq!(export["trips"][0]["id"], "kept");
    assert!(export["trips"][0]["vehicle_id"].is_null());
}

#[tokio::test]
async fn fleet_summary_groups_by_vehicle_and_month_for_managers_only() {
    let state = state();
    let (_, driver) = create_user(&state, "Aroha", "driver").await;
    let (_, manager) = create_user(&state, "Mira", "manager").await;
    let van = assigned_vehicle(&state, &[]).await;
    send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [
            on_vehicle(trip("f-1", "2026-10-01", 1_000.0), &van),
            on_vehicle(trip("f-2", "2026-10-02", 2_000.0), &van),
            trip("f-3", "2026-10-03", 500.0)
        ] })),
        true,
    )
    .await;

    let (status, _) = send_as(&state, &driver, Method::GET, "/v1/fleet/summary", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = send_as(&state, &manager, Method::GET, "/v1/fleet/summary", None).await;
    assert_eq!(status, StatusCode::OK);
    let rows = body["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{body}");
    let registered = rows
        .iter()
        .find(|r| r["vehicle_id"] == van.as_str())
        .unwrap();
    assert_eq!(registered["trips"], 2);
    assert_eq!(registered["billed_km"], "3.000");
    assert_eq!(registered["vehicle_name"], "Van");
    let inline = rows.iter().find(|r| r["vehicle_id"].is_null()).unwrap();
    assert_eq!(inline["trips"], 1);

    let (status, _) = send_as(
        &state,
        &manager,
        Method::GET,
        "/v1/fleet/summary?from=2026-13-01",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn alerts_list_heavy_licences_for_managers_only() {
    let state = state();
    let (_, driver) = create_user(&state, "Aroha", "driver").await;
    let (_, manager) = create_user(&state, "Mira", "manager").await;
    let (status, created) = send(
        &state,
        Method::POST,
        "/v1/vehicles",
        Some(json!({
            "name": "Log truck",
            "rate_type": "H61",
            "gross_kg": 44_000,
            "licence_bought_on": "2026-07-01"
        })),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, _) = send_as(&state, &driver, Method::GET, "/v1/alerts", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A window this wide always covers the 2026-09-06 expiry, whatever today's date is.
    let (status, body) = send_as(
        &state,
        &manager,
        Method::GET,
        "/v1/alerts?within_days=100000",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let alerts = body["alerts"].as_array().unwrap();
    assert!(
        alerts
            .iter()
            .any(|a| a["vehicle_name"] == "Log truck" && a["date"] == "2026-09-06"),
        "{body}"
    );

    let (status, _) = send_as(
        &state,
        &manager,
        Method::GET,
        "/v1/alerts?within_days=soon",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rotating_a_token_invalidates_the_old_one() {
    let state = state();
    let (id, old_token) = create_user(&state, "Aroha", "driver").await;
    let (status, _) = send_as(&state, &old_token, Method::GET, "/v1/trips", None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, rotated) = send(
        &state,
        Method::POST,
        &format!("/v1/users/{id}/token"),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    let new_token = rotated["token"].as_str().unwrap().to_owned();
    assert_ne!(new_token, old_token);

    let (status, _) = send_as(&state, &old_token, Method::GET, "/v1/trips", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send_as(&state, &new_token, Method::GET, "/v1/trips", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn only_admins_rotate_tokens_and_unknown_users_are_not_found() {
    let state = state();
    let (id, driver) = create_user(&state, "Aroha", "driver").await;
    let (_, manager) = create_user(&state, "Mira", "manager").await;

    let (status, _) = send_as(
        &state,
        &driver,
        Method::POST,
        &format!("/v1/users/{id}/token"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send_as(
        &state,
        &manager,
        Method::POST,
        &format!("/v1/users/{id}/token"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &state,
        Method::POST,
        "/v1/users/u_missing/token",
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// A device key pair, derived from a fixed seed so tests are repeatable.
fn device_key(seed: u8) -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
}

fn public_hex(key: &ed25519_dalek::SigningKey) -> String {
    key.verifying_key()
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Registers a device for the caller, returning the device ID.
async fn register_device(state: &AppState, token: &str, key: &ed25519_dalek::SigningKey) -> String {
    let (status, body) = send_as(
        state,
        token,
        Method::POST,
        "/v1/devices",
        Some(json!({ "name": "Logger", "public_key": public_hex(key) })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_owned()
}

/// Sends a trips upload signed by `key`, as a device would.
async fn signed_upload(
    state: &AppState,
    token: &str,
    device_id: &str,
    sequence: u64,
    key: &ed25519_dalek::SigningKey,
    signed_trips: &Value,
    sent_trips: &Value,
) -> (StatusCode, Value) {
    use ed25519_dalek::Signer;
    let trips_json = serde_json::to_string(signed_trips).unwrap();
    let message = tpt_ruc_backend::devices::signing_message(device_id, sequence, &trips_json);
    let signature: String = key
        .sign(&message)
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let body = json!({ "trips": sent_trips });
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/trips")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-device-id", device_id)
        .header("x-device-sequence", sequence.to_string())
        .header("x-device-signature", signature)
        .body(Bytes::from(body.to_string()))
        .unwrap();
    let response = handle(state, request).await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn a_signed_upload_is_recorded_against_the_device_user() {
    let state = state();
    let (driver_id, driver) = create_user(&state, "Aroha", "driver").await;
    let van = assigned_vehicle(&state, &[&driver_id]).await;
    let key = device_key(1);
    let device = register_device(&state, &driver, &key).await;

    let trips = json!([on_vehicle(trip("signed-1", "2026-10-01", 1_000.0), &van)]);
    let (status, body) = signed_upload(&state, &driver, &device, 1, &key, &trips, &trips).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["results"][0]["status"], "created");

    let (_, export) = send(&state, Method::GET, "/v1/export", None, true).await;
    assert_eq!(export["trips"][0]["user_id"], driver_id.as_str());
}

#[tokio::test]
async fn replayed_and_older_sequence_numbers_are_refused() {
    let state = state();
    let (driver_id, driver) = create_user(&state, "Aroha", "driver").await;
    let van = assigned_vehicle(&state, &[&driver_id]).await;
    let key = device_key(2);
    let device = register_device(&state, &driver, &key).await;
    let first = json!([on_vehicle(trip("seq-1", "2026-10-01", 1_000.0), &van)]);
    let second = json!([on_vehicle(trip("seq-2", "2026-10-02", 1_000.0), &van)]);

    assert_eq!(
        signed_upload(&state, &driver, &device, 5, &key, &first, &first)
            .await
            .0,
        StatusCode::OK
    );
    let (status, _) = signed_upload(&state, &driver, &device, 5, &key, &first, &first).await;
    assert_eq!(status, StatusCode::CONFLICT, "a replay must be refused");
    let (status, _) = signed_upload(&state, &driver, &device, 4, &key, &second, &second).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "an older sequence must be refused"
    );
    let (status, _) = signed_upload(&state, &driver, &device, 6, &key, &second, &second).await;
    assert_eq!(status, StatusCode::OK, "a newer sequence is accepted");
}

#[tokio::test]
async fn a_wrong_key_or_a_changed_body_is_refused() {
    let state = state();
    let (driver_id, driver) = create_user(&state, "Aroha", "driver").await;
    let van = assigned_vehicle(&state, &[&driver_id]).await;
    let key = device_key(3);
    let device = register_device(&state, &driver, &key).await;
    let trips = json!([on_vehicle(trip("sig-1", "2026-10-01", 1_000.0), &van)]);
    let tampered = json!([on_vehicle(trip("sig-1", "2026-10-01", 9_000.0), &van)]);

    let (status, _) =
        signed_upload(&state, &driver, &device, 1, &device_key(99), &trips, &trips).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "signed with another key");
    let (status, _) = signed_upload(&state, &driver, &device, 1, &key, &trips, &tampered).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "signed over another body");
    let (_, export) = send(&state, Method::GET, "/v1/export", None, true).await;
    assert_eq!(export["trips"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn revoked_devices_are_refused_and_only_their_owner_may_revoke() {
    let state = state();
    let (aroha_id, aroha) = create_user(&state, "Aroha", "driver").await;
    let (_, ben) = create_user(&state, "Ben", "driver").await;
    let van = assigned_vehicle(&state, &[&aroha_id]).await;
    let key = device_key(4);
    let device = register_device(&state, &aroha, &key).await;

    let (status, _) = send_as(
        &state,
        &ben,
        Method::DELETE,
        &format!("/v1/devices/{device}"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another driver cannot revoke it"
    );

    let (status, _) = send_as(
        &state,
        &aroha,
        Method::DELETE,
        &format!("/v1/devices/{device}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let trips = json!([on_vehicle(trip("rev-1", "2026-10-01", 1_000.0), &van)]);
    let (status, _) = signed_upload(&state, &aroha, &device, 1, &key, &trips, &trips).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn partial_device_headers_and_foreign_devices_are_refused() {
    let state = state();
    let (aroha_id, aroha) = create_user(&state, "Aroha", "driver").await;
    let (_, ben) = create_user(&state, "Ben", "driver").await;
    let van = assigned_vehicle(&state, &[&aroha_id]).await;
    let key = device_key(5);
    let device = register_device(&state, &aroha, &key).await;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/trips")
        .header(header::AUTHORIZATION, format!("Bearer {aroha}"))
        .header("x-device-id", device.as_str())
        .body(Bytes::from(json!({ "trips": [] }).to_string()))
        .unwrap();
    assert_eq!(
        handle(&state, request).await.status(),
        StatusCode::BAD_REQUEST
    );

    let trips = json!([on_vehicle(trip("foreign-1", "2026-10-01", 1_000.0), &van)]);
    let (status, _) = signed_upload(&state, &ben, &device, 1, &key, &trips, &trips).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn device_registration_checks_the_key_and_the_owner() {
    let state = state();
    let (aroha_id, aroha) = create_user(&state, "Aroha", "driver").await;

    let (status, _) = send_as(
        &state,
        &aroha,
        Method::POST,
        "/v1/devices",
        Some(json!({ "name": "X", "public_key": "not-hex" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let other_user = format!("{aroha_id}x");
    let (status, _) = send_as(
        &state,
        &aroha,
        Method::POST,
        "/v1/devices",
        Some(
            json!({ "name": "X", "public_key": public_hex(&device_key(6)), "user_id": other_user }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a driver may not register for someone else"
    );

    let (status, _) = send(
        &state,
        Method::POST,
        "/v1/devices",
        Some(json!({ "name": "X", "public_key": public_hex(&device_key(6)) })),
        true,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "the owner key must name the user"
    );
}

#[tokio::test]
async fn the_monthly_pdf_is_a_pdf_for_managers_only() {
    let state = state();
    let (_, driver) = create_user(&state, "Aroha", "driver").await;
    let (_, manager) = create_user(&state, "Mira", "manager").await;
    send(
        &state,
        Method::POST,
        "/v1/trips",
        Some(json!({ "trips": [trip("pdf-1", "2026-10-01", 1_000.0)] })),
        true,
    )
    .await;

    let request = |token: &str| {
        Request::builder()
            .method(Method::GET)
            .uri("/v1/reports/monthly.pdf")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Bytes::new())
            .unwrap()
    };
    let refused = handle(&state, request(&driver)).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    let response = handle(&state, request(&manager)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/pdf");
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename=\"monthly.pdf\""
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.ends_with(b"%%EOF\n") || bytes.windows(5).any(|w| w == b"%%EOF"));
}
