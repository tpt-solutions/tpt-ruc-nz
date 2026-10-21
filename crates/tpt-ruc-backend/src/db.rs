// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! SQLite access and schema migrations.
//!
//! SQLite allows one writer at a time, so a single connection behind a mutex is
//! enough for a self-hosted server. Each call runs on a blocking thread so the
//! async runtime is never held up by disk I/O.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{params, Connection};

use crate::error::ApiError;

/// Schema migrations in the order they must run. Each one runs once, inside a transaction.
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (1, "init", include_str!("../migrations/0001_init.sql")),
    (2, "users", include_str!("../migrations/0002_users.sql")),
    (
        3,
        "vehicles",
        include_str!("../migrations/0003_vehicles.sql"),
    ),
    (4, "devices", include_str!("../migrations/0004_devices.sql")),
    (
        5,
        "scoped_trip_ids",
        include_str!("../migrations/0005_scoped_trip_ids.sql"),
    ),
    (
        6,
        "eruc_jobs",
        include_str!("../migrations/0006_eruc_jobs.sql"),
    ),
    (
        7,
        "sessions",
        include_str!("../migrations/0007_sessions.sql"),
    ),
    (8, "raw_gps", include_str!("../migrations/0008_raw_gps.sql")),
];

/// A shared handle to the SQLite database.
#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    /// Opens the database named by `url`, creating it if needed, and applies pending migrations.
    ///
    /// Accepts `sqlite://<path>` or `:memory:`. Parent directories are created.
    pub fn open(url: &str) -> Result<Self, rusqlite::Error> {
        let connection = if url == ":memory:" {
            Connection::open_in_memory()?
        } else {
            let path = url.strip_prefix("sqlite://").unwrap_or(url);
            if let Some(parent) = Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(error))
                    })?;
                }
            }
            Connection::open(path)?
        };
        connection.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
        )?;
        migrate(&connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Runs `work` against the connection on a blocking thread.
    pub async fn run<T, F>(&self, work: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, ApiError> + Send + 'static,
    {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut guard = lock(&connection);
            work(&mut guard)
        })
        .await
        .map_err(|error| {
            tracing::error!(%error, "database task failed");
            ApiError::Internal
        })?
    }

    /// Checks that the database answers a trivial query.
    pub async fn ping(&self) -> Result<(), ApiError> {
        self.run(|connection| {
            connection.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))?;
            Ok(())
        })
        .await
    }
}

/// Locks the connection, recovering from a poisoned lock. A panic in one request
/// should not stop every later request.
fn lock(connection: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    connection
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Applies every migration that has not run yet.
pub fn migrate(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY NOT NULL,
            name       TEXT    NOT NULL,
            applied_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )?;
    for (version, name, sql) in MIGRATIONS {
        let applied: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            params![version],
            |row| row.get(0),
        )?;
        if applied {
            continue;
        }
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            params![version, name],
        )?;
        transaction.commit()?;
        tracing::info!(version, name, "applied migration");
    }
    Ok(())
}

/// Lists the versions of the migrations applied so far, in order.
pub fn applied_versions(connection: &Connection) -> Result<Vec<i64>, rusqlite::Error> {
    let mut statement =
        connection.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let versions = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database as version 1 of the server left it, with one stored trip.
    fn version_one_database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY NOT NULL, \
                 name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT ('now'));",
            )
            .unwrap();
        connection.execute_batch(MIGRATIONS[0].2).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name) VALUES (1, 'init')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO trips (id, trip_date, rate_type, gross_kg, electric, metres, \
                 rate_version, billed_metres, incl_gst_micros, excl_gst_micros, gst_micros) \
                 VALUES ('old-1', '2026-10-01', '1', 3000, 0, 1000.0, 'v', 1000, 76000, 66087, 9913)",
                [],
            )
            .unwrap();
        connection
    }

    #[test]
    fn upgrading_keeps_existing_trips_with_no_user() {
        let connection = version_one_database();
        migrate(&connection).unwrap();

        assert_eq!(
            applied_versions(&connection).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
        let (count, owner): (i64, Option<String>) = connection
            .query_row("SELECT COUNT(*), MAX(user_id) FROM trips", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(owner, None);
    }

    #[test]
    fn migrating_twice_applies_each_version_once() {
        let connection = version_one_database();
        migrate(&connection).unwrap();
        migrate(&connection).unwrap();
        assert_eq!(
            applied_versions(&connection).unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
    }
}
