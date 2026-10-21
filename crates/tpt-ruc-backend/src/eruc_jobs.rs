// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Push jobs: sending an eRUC export to a provider, with retries and an audit log.
//!
//! A manager queues a job for a date range with `POST /v1/eruc/jobs`. The
//! worker picks up queued jobs whose time has come, builds the batch from the
//! stored trips, and hands it to the configured [`PushProvider`]. A retryable
//! failure is tried again after a delay that doubles each time, up to
//! [`MAX_ATTEMPTS`]. Every change of state is written to `eruc_audit`, so a
//! job's history can be read after it has finished.
//!
//! Pushing is off unless `TPT_RUC_ERUC_PROVIDER` names a provider. Without one,
//! job requests are refused with `503`, and the worker does not run.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::Full;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::Principal;
use crate::error::ApiError;
use crate::eruc::{load_batch, PushError, PushProvider};
use crate::trips::parse_date;
use crate::{json_response, AppState};

/// Attempts made before a job is marked failed for good.
pub const MAX_ATTEMPTS: u32 = 5;
/// Delay before the first retry. Each later retry waits twice as long.
const BASE_BACKOFF_SECS: i64 = 60;
/// The longest a retry is ever delayed.
const MAX_BACKOFF_SECS: i64 = 6 * 60 * 60;
/// Jobs picked up in one pass of the worker.
const BATCH_SIZE: i64 = 10;
/// Most recent jobs returned by the list endpoint.
const LIST_LIMIT: i64 = 100;

const JOB_COLUMNS: &str = "id, provider, from_date, to_date, status, attempts, \
    next_attempt_at, last_error, created_at, updated_at";

/// The current Unix time, in whole seconds.
pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// How long to wait before the next attempt, after `attempts` failed attempts.
pub fn backoff_secs(attempts: u32) -> i64 {
    let shift = attempts.saturating_sub(1).min(30);
    (BASE_BACKOFF_SECS << shift).min(MAX_BACKOFF_SECS)
}

/// One push job, as the API returns it.
#[derive(Debug, Clone, Serialize)]
pub struct Job {
    /// The job's ID.
    pub id: i64,
    /// The provider the batch is pushed to.
    pub provider: String,
    /// First trip date in the batch, inclusive.
    pub from: String,
    /// Last trip date in the batch, inclusive.
    pub to: String,
    /// `queued`, `running`, `succeeded` or `failed`.
    pub status: String,
    /// Attempts started so far.
    pub attempts: u32,
    /// Unix time in seconds of the next attempt. Only meaningful while queued.
    pub next_attempt_at: i64,
    /// The message from the last failed attempt, cleared on success.
    pub last_error: Option<String>,
    /// Unix time in seconds the job was queued.
    pub created_at: i64,
    /// Unix time in seconds of the last change.
    pub updated_at: i64,
}

/// One line of a job's audit log.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    /// Unix time in seconds.
    pub at: i64,
    /// What happened: `queued`, `started`, `succeeded`, `retry scheduled`, `failed` or `recovered`.
    pub event: String,
    /// Details, such as the provider's note or the reason for a failure.
    pub detail: String,
}

/// What happened in one attempt, before it is written down.
enum Outcome {
    Succeeded(String),
    Retry(String),
    Failed(String),
}

fn job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    Ok(Job {
        id: row.get(0)?,
        provider: row.get(1)?,
        from: row.get(2)?,
        to: row.get(3)?,
        status: row.get(4)?,
        attempts: row.get(5)?,
        next_attempt_at: row.get(6)?,
        last_error: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn load_job(connection: &Connection, id: i64) -> rusqlite::Result<Option<Job>> {
    connection
        .query_row(
            &format!("SELECT {JOB_COLUMNS} FROM eruc_jobs WHERE id = ?1"),
            params![id],
            job_row,
        )
        .optional()
}

fn audit(
    connection: &Connection,
    job_id: i64,
    at: i64,
    event: &str,
    detail: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO eruc_audit (job_id, at, event, detail) VALUES (?1, ?2, ?3, ?4)",
        params![job_id, at, event, detail],
    )?;
    Ok(())
}

/// Adds a queued job and its first audit line. Returns the job's ID.
fn enqueue(
    connection: &Connection,
    provider: &str,
    from: &str,
    to: &str,
    now: i64,
    requested_by: &str,
) -> rusqlite::Result<i64> {
    connection.execute(
        "INSERT INTO eruc_jobs (provider, from_date, to_date, status, attempts, \
         next_attempt_at, created_at, updated_at) VALUES (?1, ?2, ?3, 'queued', 0, ?4, ?4, ?4)",
        params![provider, from, to, now],
    )?;
    let id = connection.last_insert_rowid();
    audit(
        connection,
        id,
        now,
        "queued",
        &format!("requested by {requested_by}"),
    )?;
    Ok(id)
}

/// Marks the queued jobs for `provider` that are due as running, and counts the attempt.
fn claim_due(connection: &Connection, provider: &str, now: i64) -> rusqlite::Result<Vec<Job>> {
    let tx = connection.unchecked_transaction()?;
    let mut due = {
        let mut statement = tx.prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM eruc_jobs WHERE status = 'queued' AND provider = ?1 \
             AND next_attempt_at <= ?2 ORDER BY next_attempt_at, id LIMIT ?3"
        ))?;
        let rows = statement.query_map(params![provider, now, BATCH_SIZE], job_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for job in &mut due {
        job.attempts += 1;
        job.status = "running".to_owned();
        job.updated_at = now;
        tx.execute(
            "UPDATE eruc_jobs SET status = 'running', attempts = ?2, updated_at = ?3 WHERE id = ?1",
            params![job.id, job.attempts, now],
        )?;
        audit(
            &tx,
            job.id,
            now,
            "started",
            &format!("attempt {}", job.attempts),
        )?;
    }
    tx.commit()?;
    Ok(due)
}

/// Writes down the result of an attempt and sets the job's next state.
fn finish(
    connection: &Connection,
    job_id: i64,
    attempts: u32,
    now: i64,
    outcome: Outcome,
) -> rusqlite::Result<()> {
    match outcome {
        Outcome::Succeeded(note) => {
            connection.execute(
                "UPDATE eruc_jobs SET status = 'succeeded', last_error = NULL, updated_at = ?2 \
                 WHERE id = ?1",
                params![job_id, now],
            )?;
            audit(connection, job_id, now, "succeeded", &note)
        }
        Outcome::Retry(message) => {
            let delay = backoff_secs(attempts);
            connection.execute(
                "UPDATE eruc_jobs SET status = 'queued', last_error = ?2, \
                 next_attempt_at = ?3, updated_at = ?4 WHERE id = ?1",
                params![job_id, message, now + delay, now],
            )?;
            audit(
                connection,
                job_id,
                now,
                "retry scheduled",
                &format!("{message}; next attempt in {delay} s"),
            )
        }
        Outcome::Failed(message) => {
            connection.execute(
                "UPDATE eruc_jobs SET status = 'failed', last_error = ?2, updated_at = ?3 \
                 WHERE id = ?1",
                params![job_id, message, now],
            )?;
            audit(connection, job_id, now, "failed", &message)
        }
    }
}

/// After a restart, jobs left running were interrupted. They go back to the queue now.
/// Returns how many were requeued.
pub fn recover_interrupted(connection: &Connection, now: i64) -> rusqlite::Result<usize> {
    let ids: Vec<i64> = {
        let mut statement =
            connection.prepare("SELECT id FROM eruc_jobs WHERE status = 'running' ORDER BY id")?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for id in &ids {
        connection.execute(
            "UPDATE eruc_jobs SET status = 'queued', next_attempt_at = ?2, updated_at = ?2 \
             WHERE id = ?1",
            params![id, now],
        )?;
        audit(
            connection,
            *id,
            now,
            "recovered",
            "the server stopped during this attempt; requeued",
        )?;
    }
    Ok(ids.len())
}

/// Runs one pass of the queue: every due job for this provider gets one attempt.
/// Returns how many jobs were attempted.
pub async fn process_due(
    state: &AppState,
    provider: &dyn PushProvider,
    now: i64,
) -> Result<usize, ApiError> {
    let provider_id = provider.id().to_owned();
    let due = state
        .db
        .run(move |connection| Ok(claim_due(connection, &provider_id, now)?))
        .await?;

    let count = due.len();
    for job in due {
        let outcome = match load_batch(state, job.from.clone(), job.to.clone()).await {
            Ok(batch) => match provider.push(&batch).await {
                Ok(note) => Outcome::Succeeded(note),
                Err(PushError::Retryable(message)) => Outcome::Retry(message),
                Err(PushError::Permanent(message)) => Outcome::Failed(message),
            },
            Err(_) => Outcome::Retry("could not read the stored trips".to_owned()),
        };
        let outcome = match outcome {
            Outcome::Retry(message) if job.attempts >= MAX_ATTEMPTS => Outcome::Failed(format!(
                "gave up after {} attempts: {message}",
                job.attempts
            )),
            other => other,
        };
        let (job_id, attempts) = (job.id, job.attempts);
        state
            .db
            .run(move |connection| Ok(finish(connection, job_id, attempts, now, outcome)?))
            .await?;
    }
    Ok(count)
}

/// Runs the queue in the background: recovers interrupted jobs once, then makes a pass every `every`.
pub async fn run_worker(state: Arc<AppState>, provider: Arc<dyn PushProvider>, every: Duration) {
    let now = now_secs();
    let recovered = state
        .db
        .run(move |connection| Ok(recover_interrupted(connection, now)?))
        .await;
    match recovered {
        Ok(0) => {}
        Ok(count) => tracing::info!(count, "requeued push jobs interrupted by a restart"),
        Err(_) => tracing::error!("could not recover interrupted push jobs"),
    }

    let mut ticker = tokio::time::interval(every);
    loop {
        ticker.tick().await;
        if process_due(&state, provider.as_ref(), now_secs())
            .await
            .is_err()
        {
            tracing::error!("eRUC push run failed");
        }
    }
}

#[derive(Deserialize)]
struct CreateJob {
    provider: String,
    from: String,
    to: String,
}

/// `POST /v1/eruc/jobs`: queues an export of `from`..`to` to the configured provider.
pub async fn create(
    state: &AppState,
    principal: &Principal,
    body: &[u8],
) -> Result<Response<Full<Bytes>>, ApiError> {
    let push = state.eruc_push.as_ref().ok_or_else(|| {
        ApiError::Unavailable(
            "no eRUC push provider is configured; set TPT_RUC_ERUC_PROVIDER".to_owned(),
        )
    })?;
    let request: CreateJob = serde_json::from_slice(body).map_err(|_| {
        ApiError::BadRequest("body must be JSON with provider, from and to".to_owned())
    })?;
    if request.provider != push.id() {
        return Err(ApiError::BadRequest(format!(
            "provider must be {}",
            push.id()
        )));
    }
    if parse_date(&request.from).is_none() || parse_date(&request.to).is_none() {
        return Err(ApiError::BadRequest(
            "from and to must be YYYY-MM-DD".to_owned(),
        ));
    }
    if request.from > request.to {
        return Err(ApiError::BadRequest("from must not be after to".to_owned()));
    }

    let requested_by = principal.user_id().unwrap_or("owner key").to_owned();
    let provider = request.provider;
    let (from, to) = (request.from, request.to);
    let id = state
        .db
        .run(move |connection| {
            Ok(enqueue(
                connection,
                &provider,
                &from,
                &to,
                now_secs(),
                &requested_by,
            )?)
        })
        .await?;
    let job = state
        .db
        .run(move |connection| Ok(load_job(connection, id)?))
        .await?
        .ok_or(ApiError::Internal)?;
    Ok(json_response(StatusCode::ACCEPTED, &job))
}

/// `GET /v1/eruc/jobs`: the most recent jobs, newest first.
pub async fn list(state: &AppState) -> Result<Response<Full<Bytes>>, ApiError> {
    let jobs = state
        .db
        .run(move |connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {JOB_COLUMNS} FROM eruc_jobs ORDER BY id DESC LIMIT ?1"
            ))?;
            let rows = statement.query_map(params![LIST_LIMIT], job_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(json_response(StatusCode::OK, &json!({ "jobs": jobs })))
}

/// `GET /v1/eruc/jobs/{id}`: one job, with its audit log.
pub async fn get(state: &AppState, id_text: &str) -> Result<Response<Full<Bytes>>, ApiError> {
    let id: i64 = id_text.parse().map_err(|_| ApiError::NotFound)?;
    let found = state
        .db
        .run(move |connection| {
            let Some(job) = load_job(connection, id)? else {
                return Ok(None);
            };
            let mut statement = connection.prepare(
                "SELECT at, event, detail FROM eruc_audit WHERE job_id = ?1 ORDER BY id",
            )?;
            let rows = statement.query_map(params![id], |row| {
                Ok(AuditEntry {
                    at: row.get(0)?,
                    event: row.get(1)?,
                    detail: row.get(2)?,
                })
            })?;
            let entries = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(Some((job, entries)))
        })
        .await?;
    let (job, audit) = found.ok_or(ApiError::NotFound)?;
    Ok(json_response(
        StatusCode::OK,
        &json!({ "job": job, "audit": audit }),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::auth::Principal as Caller;
    use crate::db::Database;
    use crate::eruc::{DryRun, ErucBatch, PushFuture};
    use crate::ratelimit::RateLimiter;

    /// Fails with a retryable error for the first `failures` calls, then succeeds.
    struct Flaky {
        calls: AtomicU32,
        failures: u32,
        permanent: bool,
    }

    impl PushProvider for Flaky {
        fn id(&self) -> &'static str {
            "flaky"
        }

        fn push<'a>(&'a self, _batch: &'a ErucBatch) -> PushFuture<'a> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let fail = call <= self.failures;
            let permanent = self.permanent;
            Box::pin(async move {
                match (fail, permanent) {
                    (false, _) => Ok("sent".to_owned()),
                    (true, true) => Err(PushError::Permanent(format!("refused on call {call}"))),
                    (true, false) => Err(PushError::Retryable(format!("call {call} failed"))),
                }
            })
        }
    }

    fn state_with(push: Option<Arc<dyn PushProvider>>) -> AppState {
        AppState {
            db: Database::open(":memory:").unwrap(),
            api_key: "0123456789abcdef0123456789abcdef".to_owned(),
            cors_origins: Vec::new(),
            limiter: RateLimiter::default(),
            pwa_dir: None,
            eruc_push: push,
        }
    }

    async fn queue(state: &AppState, provider: &str, now: i64) -> i64 {
        let provider = provider.to_owned();
        state
            .db
            .run(move |connection| {
                Ok(enqueue(
                    connection,
                    &provider,
                    "2026-10-01",
                    "2026-10-31",
                    now,
                    "test",
                )?)
            })
            .await
            .unwrap()
    }

    async fn job(state: &AppState, id: i64) -> Job {
        state
            .db
            .run(move |connection| Ok(load_job(connection, id)?))
            .await
            .unwrap()
            .unwrap()
    }

    async fn events(state: &AppState, id: i64) -> Vec<String> {
        state
            .db
            .run(move |connection| {
                let mut statement = connection
                    .prepare("SELECT event FROM eruc_audit WHERE job_id = ?1 ORDER BY id")?;
                let rows = statement.query_map(params![id], |row| row.get(0))?;
                Ok(rows.collect::<Result<Vec<String>, _>>()?)
            })
            .await
            .unwrap()
    }

    #[test]
    fn backoff_doubles_and_is_capped() {
        assert_eq!(backoff_secs(1), 60);
        assert_eq!(backoff_secs(2), 120);
        assert_eq!(backoff_secs(5), 960);
        assert_eq!(backoff_secs(30), MAX_BACKOFF_SECS);
    }

    #[tokio::test]
    async fn retryable_failure_waits_then_succeeds() {
        let flaky = Flaky {
            calls: AtomicU32::new(0),
            failures: 1,
            permanent: false,
        };
        let state = state_with(None);
        let id = queue(&state, "flaky", 1_000).await;

        assert_eq!(process_due(&state, &flaky, 1_000).await.unwrap(), 1);
        let after_failure = job(&state, id).await;
        assert_eq!(after_failure.status, "queued");
        assert_eq!(after_failure.attempts, 1);
        assert_eq!(after_failure.next_attempt_at, 1_060);
        assert_eq!(after_failure.last_error.as_deref(), Some("call 1 failed"));

        assert_eq!(process_due(&state, &flaky, 1_059).await.unwrap(), 0);
        assert_eq!(process_due(&state, &flaky, 1_060).await.unwrap(), 1);
        let done = job(&state, id).await;
        assert_eq!(done.status, "succeeded");
        assert_eq!(done.attempts, 2);
        assert_eq!(done.last_error, None);
        assert_eq!(
            events(&state, id).await,
            [
                "queued",
                "started",
                "retry scheduled",
                "started",
                "succeeded"
            ]
        );
    }

    #[tokio::test]
    async fn repeated_failures_end_in_failed_after_max_attempts() {
        let flaky = Flaky {
            calls: AtomicU32::new(0),
            failures: u32::MAX,
            permanent: false,
        };
        let state = state_with(None);
        let id = queue(&state, "flaky", 1_000).await;

        let mut now = 1_000;
        for _ in 0..MAX_ATTEMPTS {
            process_due(&state, &flaky, now).await.unwrap();
            now = job(&state, id).await.next_attempt_at;
        }
        let finished = job(&state, id).await;
        assert_eq!(finished.status, "failed");
        assert_eq!(finished.attempts, MAX_ATTEMPTS);
        assert!(finished
            .last_error
            .unwrap()
            .starts_with("gave up after 5 attempts"));
        assert_eq!(
            process_due(&state, &flaky, now + 1_000_000).await.unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn permanent_failure_is_not_retried() {
        let flaky = Flaky {
            calls: AtomicU32::new(0),
            failures: u32::MAX,
            permanent: true,
        };
        let state = state_with(None);
        let id = queue(&state, "flaky", 1_000).await;

        process_due(&state, &flaky, 1_000).await.unwrap();
        assert_eq!(job(&state, id).await.status, "failed");
        assert_eq!(process_due(&state, &flaky, 10_000_000).await.unwrap(), 0);
        assert_eq!(flaky.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn jobs_for_other_providers_are_left_alone() {
        let state = state_with(None);
        let id = queue(&state, "someone-else", 1_000).await;
        assert_eq!(process_due(&state, &DryRun, 1_000).await.unwrap(), 0);
        assert_eq!(job(&state, id).await.status, "queued");
    }

    #[tokio::test]
    async fn dry_run_succeeds_and_says_nothing_was_sent() {
        let state = state_with(None);
        let id = queue(&state, "dry-run", 1_000).await;
        process_due(&state, &DryRun, 1_000).await.unwrap();
        let done = job(&state, id).await;
        assert_eq!(done.status, "succeeded");
        let detail: String = state
            .db
            .run(move |connection| {
                Ok(connection.query_row(
                    "SELECT detail FROM eruc_audit WHERE job_id = ?1 AND event = 'succeeded'",
                    params![id],
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        assert!(detail.contains("not sent"), "{detail}");
    }

    #[tokio::test]
    async fn interrupted_jobs_are_requeued_on_recovery() {
        let state = state_with(None);
        let id = queue(&state, "dry-run", 1_000).await;
        state
            .db
            .run(move |connection| {
                connection.execute(
                    "UPDATE eruc_jobs SET status = 'running' WHERE id = ?1",
                    params![id],
                )?;
                Ok(recover_interrupted(connection, 2_000)?)
            })
            .await
            .unwrap();
        let back = job(&state, id).await;
        assert_eq!(back.status, "queued");
        assert_eq!(back.next_attempt_at, 2_000);
        assert_eq!(events(&state, id).await.last().unwrap(), "recovered");
    }

    #[tokio::test]
    async fn create_needs_a_provider_and_valid_dates() {
        let caller = Caller::Owner;
        let body = br#"{"provider":"dry-run","from":"2026-10-01","to":"2026-10-31"}"#;
        let unconfigured = state_with(None);
        let error = create(&unconfigured, &caller, body).await.unwrap_err();
        assert!(matches!(error, ApiError::Unavailable(_)));

        let state = state_with(Some(Arc::new(DryRun)));
        let bad = br#"{"provider":"other","from":"2026-10-01","to":"2026-10-31"}"#;
        assert!(matches!(
            create(&state, &caller, bad).await,
            Err(ApiError::BadRequest(_))
        ));
        let reversed = br#"{"provider":"dry-run","from":"2026-10-31","to":"2026-10-01"}"#;
        assert!(matches!(
            create(&state, &caller, reversed).await,
            Err(ApiError::BadRequest(_))
        ));
        let accepted = create(&state, &caller, body).await.unwrap();
        assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    }
}
