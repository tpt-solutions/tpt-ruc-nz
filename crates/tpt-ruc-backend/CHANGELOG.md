# Changelog

All notable changes to `tpt-ruc-backend` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). The workspace-wide history is in the [root changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

- Self-hostable HTTP server with trip ingest and re-validation through `tpt-ruc-core`, CSV reports for NZTA audits, trip export, and date-range deletion.
- Users with roles (`driver`, `manager`, `admin`), session tokens with rotation, and per-IP rate limits.
- SQLite storage with migrations applied at start-up.
- Owner API key, generated and saved on first start when `TPT_RUC_API_KEY` is not set.
- `tpt-ruc-backend healthcheck` subcommand, plus the open `/healthz` and `/readyz` endpoints.
- Optional serving of the built web app from `TPT_RUC_PWA_DIR`, with path checks. Static assets are exempt from the API rate limit.
- Optional trip retention through `TPT_RUC_RETENTION_DAYS`.
- Vehicles and driver assignment, a fleet summary, and heavy-licence expiry alerts.
- Device keys with signed uploads and replay protection.
- A monthly PDF report, `/v1/reports/monthly.pdf`.
- An OpenAPI description in `docs/api/openapi.yaml`.
- Generic eRUC export at `GET /v1/eruc/export` (`generic-json` and `generic-csv`, manager-only), with an `ErucProvider` adapter trait. No provider is pushed to yet.
- An optional eRUC push queue, off unless `TPT_RUC_ERUC_PROVIDER` is set. Jobs are created, listed and inspected under `/v1/eruc/jobs`, with retries, a per-job audit log and recovery after a restart. The only provider is the `dry-run` placeholder, which sends nothing.
- Docker: a multi-stage `Dockerfile` and `docker-compose.yml`. `docker compose up -d` starts the server on `127.0.0.1:8080`.
