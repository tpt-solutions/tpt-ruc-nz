// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Vehicles and which drivers may use them.
//!
//! Managers and admins maintain the fleet. Drivers can list the vehicles
//! assigned to them and nothing else. A vehicle a driver cannot see is reported
//! as not found, so the response does not reveal that it exists.

use bytes::Bytes;
use http::{Method, Request, Response, StatusCode};
use http_body_util::Full;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::{random_hex, Principal, Role};
use crate::error::ApiError;
use crate::trips::{format_date, known_type_code, parse_date};
use crate::{authorize, json_response, AppState};

/// Longest vehicle name accepted, in bytes.
const MAX_NAME_LEN: usize = 100;
/// Most drivers that may be assigned to one vehicle in a single request.
const MAX_DRIVERS: usize = 200;

/// The rate fields of a registered vehicle, as trip ingest needs them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleRecord {
    /// NZTA RUC vehicle type code.
    pub rate_type: String,
    /// RUC weight in kilograms.
    pub gross_kg: u32,
    /// True for a battery electric vehicle.
    pub electric: bool,
}

/// Looks up a vehicle the caller may use. A driver may only use vehicles
/// assigned to them. Other roles may use any vehicle. Returns `None` when the
/// vehicle does not exist or the caller may not see it.
pub fn visible_vehicle(
    connection: &Connection,
    vehicle_id: &str,
    driver_user: Option<&str>,
) -> rusqlite::Result<Option<VehicleRecord>> {
    connection
        .query_row(
            "SELECT rate_type, gross_kg, electric FROM vehicles \
             WHERE id = ?1 AND (?2 IS NULL OR id IN \
             (SELECT vehicle_id FROM vehicle_drivers WHERE user_id = ?2))",
            params![vehicle_id, driver_user],
            |row| {
                let gross: i64 = row.get(1)?;
                Ok(VehicleRecord {
                    rate_type: row.get(0)?,
                    gross_kg: u32::try_from(gross).unwrap_or(0),
                    electric: row.get(2)?,
                })
            },
        )
        .optional()
}

#[derive(Debug, Deserialize)]
struct VehicleInput {
    name: String,
    rate_type: String,
    gross_kg: u32,
    #[serde(default)]
    electric: bool,
    #[serde(default)]
    licence_bought_on: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriversInput {
    user_ids: Vec<String>,
}

/// A vehicle whose details passed validation.
struct ValidVehicle {
    name: String,
    rate_type: String,
    gross_kg: u32,
    electric: bool,
    licence_bought_on: Option<String>,
}

/// One vehicle as the API returns it.
#[derive(Debug, Serialize)]
struct VehicleView {
    id: String,
    name: String,
    rate_type: String,
    gross_kg: i64,
    electric: bool,
    licence_bought_on: Option<String>,
    driver_ids: Vec<String>,
    created_at: String,
}

/// Routes `/v1/vehicles` and everything below it.
pub async fn route(state: &AppState, request: &Request<Bytes>) -> Response<Full<Bytes>> {
    let rest = request
        .uri()
        .path()
        .strip_prefix("/v1/vehicles")
        .unwrap_or_default();
    let segments: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    let method = request.method().clone();

    let result = match segments.as_slice() {
        [] => {
            let allowed = [Method::GET, Method::POST];
            match authorize(state, request, &allowed).await {
                Ok(principal) if method == Method::POST => {
                    create(state, &principal, request.body()).await
                }
                Ok(principal) => list(state, &principal).await,
                Err(response) => return *response,
            }
        }
        [id] => {
            let allowed = [Method::GET, Method::PUT, Method::DELETE];
            match authorize(state, request, &allowed).await {
                Ok(principal) if method == Method::PUT => {
                    update(state, &principal, id, request.body()).await
                }
                Ok(principal) if method == Method::DELETE => delete(state, &principal, id).await,
                Ok(principal) => get(state, &principal, id).await,
                Err(response) => return *response,
            }
        }
        [id, "drivers"] => {
            let allowed = [Method::PUT];
            match authorize(state, request, &allowed).await {
                Ok(principal) => set_drivers(state, &principal, id, request.body()).await,
                Err(response) => return *response,
            }
        }
        _ => return ApiError::NotFound.into_response(),
    };
    result.unwrap_or_else(|error| error.into_response())
}

/// `POST /v1/vehicles`: adds a vehicle.
async fn create(
    state: &AppState,
    principal: &Principal,
    body: &Bytes,
) -> Result<Response<Full<Bytes>>, ApiError> {
    principal.require(Role::Manager)?;
    let vehicle = parse_vehicle(body)?;
    let id = format!("v_{}", random_hex(12)?);
    {
        let id = id.clone();
        state
            .db
            .run(move |connection| {
                connection.execute(
                    "INSERT INTO vehicles (id, name, rate_type, gross_kg, electric, licence_bought_on) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        id,
                        vehicle.name,
                        vehicle.rate_type,
                        i64::from(vehicle.gross_kg),
                        vehicle.electric,
                        vehicle.licence_bought_on
                    ],
                )?;
                Ok(())
            })
            .await?;
    }
    let view = view_by_id(state, &id).await?;
    Ok(json_response(StatusCode::CREATED, &view))
}

/// `GET /v1/vehicles`: managers and admins see every vehicle, drivers see their own.
async fn list(state: &AppState, principal: &Principal) -> Result<Response<Full<Bytes>>, ApiError> {
    let scope = principal.trip_scope().map(str::to_owned);
    let vehicles = state
        .db
        .run(move |connection| {
            let ids: Vec<String> = {
                let mut statement = connection.prepare(
                    "SELECT id FROM vehicles WHERE (?1 IS NULL OR id IN \
                     (SELECT vehicle_id FROM vehicle_drivers WHERE user_id = ?1)) \
                     ORDER BY created_at, id",
                )?;
                let rows = statement.query_map(params![scope], |row| row.get(0))?;
                rows.collect::<Result<_, _>>()?
            };
            ids.iter()
                .map(|id| view_in(connection, id))
                .collect::<Result<Vec<_>, _>>()
                .map_err(ApiError::from)
        })
        .await?;
    Ok(json_response(
        StatusCode::OK,
        &json!({ "vehicles": vehicles }),
    ))
}

/// `GET /v1/vehicles/{id}`.
async fn get(
    state: &AppState,
    principal: &Principal,
    id: &str,
) -> Result<Response<Full<Bytes>>, ApiError> {
    let scope = principal.trip_scope().map(str::to_owned);
    let id = id.to_owned();
    let view = state
        .db
        .run(move |connection| {
            if visible_vehicle(connection, &id, scope.as_deref())?.is_none() {
                return Err(ApiError::NotFound);
            }
            view_in(connection, &id).map_err(ApiError::from)
        })
        .await?;
    Ok(json_response(StatusCode::OK, &view))
}

/// `PUT /v1/vehicles/{id}`: replaces a vehicle's details.
async fn update(
    state: &AppState,
    principal: &Principal,
    id: &str,
    body: &Bytes,
) -> Result<Response<Full<Bytes>>, ApiError> {
    principal.require(Role::Manager)?;
    let vehicle = parse_vehicle(body)?;
    let id = id.to_owned();
    {
        let id = id.clone();
        let changed = state
            .db
            .run(move |connection| {
                Ok(connection.execute(
                    "UPDATE vehicles SET name = ?2, rate_type = ?3, gross_kg = ?4, electric = ?5, \
                     licence_bought_on = ?6 WHERE id = ?1",
                    params![
                        id,
                        vehicle.name,
                        vehicle.rate_type,
                        i64::from(vehicle.gross_kg),
                        vehicle.electric,
                        vehicle.licence_bought_on
                    ],
                )?)
            })
            .await?;
        if changed == 0 {
            return Err(ApiError::NotFound);
        }
    }
    let view = view_by_id(state, &id).await?;
    Ok(json_response(StatusCode::OK, &view))
}

/// `DELETE /v1/vehicles/{id}`. The vehicle's trips are kept, without a vehicle.
async fn delete(
    state: &AppState,
    principal: &Principal,
    id: &str,
) -> Result<Response<Full<Bytes>>, ApiError> {
    principal.require(Role::Manager)?;
    let id = id.to_owned();
    let removed = state
        .db
        .run(move |connection| {
            Ok(connection.execute("DELETE FROM vehicles WHERE id = ?1", params![id])?)
        })
        .await?;
    if removed == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(json_response(StatusCode::OK, &json!({ "deleted": true })))
}

/// `PUT /v1/vehicles/{id}/drivers`: replaces the set of drivers assigned to a vehicle.
async fn set_drivers(
    state: &AppState,
    principal: &Principal,
    id: &str,
    body: &Bytes,
) -> Result<Response<Full<Bytes>>, ApiError> {
    principal.require(Role::Manager)?;
    let input: DriversInput = serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    if input.user_ids.len() > MAX_DRIVERS {
        return Err(ApiError::BadRequest(format!(
            "at most {MAX_DRIVERS} drivers per vehicle"
        )));
    }
    let count = input.user_ids.len();
    let id = id.to_owned();
    let view = state
        .db
        .run(move |connection| {
            let transaction = connection.transaction()?;
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM vehicles WHERE id = ?1)",
                params![id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(ApiError::NotFound);
            }
            transaction.execute(
                "DELETE FROM vehicle_drivers WHERE vehicle_id = ?1",
                params![id],
            )?;
            for user_id in &input.user_ids {
                let known: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM users WHERE id = ?1)",
                    params![user_id],
                    |row| row.get(0),
                )?;
                if !known {
                    return Err(ApiError::BadRequest(format!("unknown user {user_id}")));
                }
                transaction.execute(
                    "INSERT OR IGNORE INTO vehicle_drivers (vehicle_id, user_id) VALUES (?1, ?2)",
                    params![id, user_id],
                )?;
            }
            transaction.commit()?;
            view_in(connection, &id).map_err(ApiError::from)
        })
        .await?;
    tracing::info!(vehicle = %view.id, drivers = count, "vehicle drivers replaced");
    Ok(json_response(StatusCode::OK, &view))
}

/// Reads and checks a vehicle from a request body.
fn parse_vehicle(body: &Bytes) -> Result<ValidVehicle, ApiError> {
    let input: VehicleInput = serde_json::from_slice(body)
        .map_err(|error| ApiError::BadRequest(format!("invalid JSON: {error}")))?;
    let name = input.name.trim().to_owned();
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(ApiError::BadRequest(format!(
            "name must be 1 to {MAX_NAME_LEN} characters"
        )));
    }
    if known_type_code(&input.rate_type).is_none() {
        return Err(ApiError::BadRequest(format!(
            "unknown vehicle type code {}",
            input.rate_type
        )));
    }
    if input.gross_kg == 0 {
        return Err(ApiError::BadRequest(
            "gross_kg must be above zero".to_owned(),
        ));
    }
    let licence_bought_on = match input.licence_bought_on {
        Some(text) => {
            let date = parse_date(&text).ok_or_else(|| {
                ApiError::BadRequest("licence_bought_on must be YYYY-MM-DD".to_owned())
            })?;
            Some(format_date(date))
        }
        None => None,
    };
    Ok(ValidVehicle {
        name,
        rate_type: input.rate_type,
        gross_kg: input.gross_kg,
        electric: input.electric,
        licence_bought_on,
    })
}

async fn view_by_id(state: &AppState, id: &str) -> Result<VehicleView, ApiError> {
    let id = id.to_owned();
    state
        .db
        .run(move |connection| view_in(connection, &id).map_err(ApiError::from))
        .await
}

/// Reads one vehicle with its assigned drivers.
fn view_in(connection: &Connection, id: &str) -> rusqlite::Result<VehicleView> {
    let mut view = connection.query_row(
        "SELECT id, name, rate_type, gross_kg, electric, licence_bought_on, created_at \
         FROM vehicles WHERE id = ?1",
        params![id],
        |row| {
            Ok(VehicleView {
                id: row.get(0)?,
                name: row.get(1)?,
                rate_type: row.get(2)?,
                gross_kg: row.get(3)?,
                electric: row.get(4)?,
                licence_bought_on: row.get(5)?,
                driver_ids: Vec::new(),
                created_at: row.get(6)?,
            })
        },
    )?;
    let mut statement = connection
        .prepare("SELECT user_id FROM vehicle_drivers WHERE vehicle_id = ?1 ORDER BY user_id")?;
    view.driver_ids = statement
        .query_map(params![id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(view)
}
