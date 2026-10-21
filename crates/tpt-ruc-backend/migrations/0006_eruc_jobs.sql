-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Copyright (c) 2026 TPT Solutions

-- Push jobs: one export of a date range to one provider. A job is queued,
-- running, succeeded or failed. next_attempt_at is a Unix time in seconds, and
-- a queued job is only picked up once that time has passed.
CREATE TABLE eruc_jobs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    provider        TEXT    NOT NULL,
    from_date       TEXT    NOT NULL,
    to_date         TEXT    NOT NULL,
    status          TEXT    NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
    attempts        INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL,
    last_error      TEXT,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX eruc_jobs_due ON eruc_jobs (status, next_attempt_at);

-- The audit log: every change of state for a job, kept after the job finishes.
CREATE TABLE eruc_audit (
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL REFERENCES eruc_jobs (id) ON DELETE CASCADE,
    at     INTEGER NOT NULL,
    event  TEXT    NOT NULL,
    detail TEXT    NOT NULL
);

CREATE INDEX eruc_audit_by_job ON eruc_audit (job_id, id);
