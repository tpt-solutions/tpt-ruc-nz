# tpt-ruc-backend

A self-hostable HTTP server for tpt-ruc-nz. It takes trips from the phone, PWA or device, checks them again with [`tpt-ruc-core`](../tpt-ruc-core/README.md), stores them in SQLite, and produces the reports an NZTA audit needs.

> **NZTA Section 43 disclaimer.** This server is an open-source reference implementation. It is **not an NZTA-approved distance recorder**, and its reports do not replace a Section 43 approved system. Rates are not authoritative until checked against [nzta.govt.nz](https://www.nzta.govt.nz). See the [project README](../../README.md).

This crate is not published to crates.io (`publish = false`). It is run from source or as a Docker container.

## Contents

- [Run it](#run-it)
- [Configuration](#configuration)
- [Users, roles and authentication](#users-roles-and-authentication)
- [API overview](#api-overview)
- [Data and backups](#data-and-backups)
- [Development](#development)
- [Licence](#licence)

## Run it

### With Docker (recommended)

```sh
docker compose up -d
docker compose logs tpt-ruc
```

The first start creates an API key and prints it in the log under "No API key was set". Every request needs it. The compose file listens only on `127.0.0.1:8080`. See the [root README](../../README.md) and [docs/guides/self-hosting.md](../../docs/guides/self-hosting.md).

### From source

Install [Rust](https://rustup.rs), then from the repository root:

```sh
cargo run -p tpt-ruc-backend
```

The data and the generated API key are kept in `./data`.

### Health check

```sh
tpt-ruc-backend healthcheck
```

The command exits 0 when a server on `TPT_RUC_BIND` is healthy. The server also exposes `/healthz` (liveness) and `/readyz` (readiness). Both are open, so an orchestrator can probe them without a key.

## Configuration

Every setting has a default, so the server starts with no configuration. Settings come from environment variables. Copy `.env.example` to `.env` to set them with Docker Compose.

| Variable | Default | Purpose |
|---|---|---|
| `TPT_RUC_API_KEY` | generated on first start | The owner's API key. Must be at least 32 characters. Leave unset to have the server generate and save one. |
| `TPT_RUC_BIND` | `127.0.0.1:8080` | Address to listen on. Loopback only by default. |
| `TPT_RUC_DATABASE_URL` | `sqlite://data/tpt-ruc.db` | SQLite connection URL. |
| `TPT_RUC_CORS_ORIGINS` | none | Comma-separated origins allowed to call the API from a browser. |
| `TPT_RUC_RETENTION_DAYS` | keep forever | Delete trips older than this many days. Whole number, at least 1. |
| `TPT_RUC_PWA_DIR` | none | Folder of the built web app. When set, the server also serves the pages. |
| `TPT_RUC_ERUC_PROVIDER` | none (sends nothing) | Enables the eRUC push queue. The only provider is the `dry-run` placeholder, which sends nothing. |
| `TPT_RUC_ALLOW_RAW_GPS` | `false` | Lets users upload raw GPS points to `POST /v1/gps/points`. Summaries only while off. |
| `TPT_RUC_CONFIG` | none | Path to a TOML file of the same settings, keyed by variable name. Environment variables override it. |

Invalid values stop the server at start-up with a message that names the variable.

## Users, roles and authentication

- The **owner's API key** always works and has full access.
- **Users** have one of three roles:
  - `driver` records trips and sees their own trips only.
  - `manager` sees every trip and runs reports.
  - `admin` does what a manager does and also manages users.
- Users sign in to get a **session token** (`/v1/sessions`). Tokens can be rotated.
- **Device keys** let a tracker sign its uploads with Ed25519. Replayed uploads are rejected.
- Per-IP **rate limits** apply to the API. Static files served from `TPT_RUC_PWA_DIR` are exempt.

## API overview

All endpoints are under `/v1` and need a bearer token, except the health endpoints. The full description is in [docs/api/openapi.yaml](../../docs/api/openapi.yaml).

| Area | Endpoints |
|---|---|
| Trips | `/v1/trips`: ingest, list, and delete by date range. |
| Export | `/v1/export`: full trip export. |
| Reports | `/v1/reports/trips.csv`, `/v1/reports/monthly.csv`, `/v1/reports/monthly.pdf`. |
| Vehicles | `/v1/vehicles`, `/v1/vehicles/{id}`: vehicles and driver assignment. |
| Fleet | `/v1/fleet/summary`. |
| Alerts | `/v1/alerts`: heavy-licence expiry alerts. |
| Users | `/v1/users`, `/v1/users/{id}`. |
| Sessions | `/v1/sessions`: sign-in and token rotation. |
| Devices | `/v1/devices`, `/v1/devices/{id}`: device keys and signed uploads. |
| eRUC | `/v1/eruc/export` (generic JSON and CSV), `/v1/eruc/jobs` (push queue). |

Requests are limited to 256 KiB. A full batch of trips is far smaller than that.

## Data and backups

- Storage is a single SQLite file. The schema is in `migrations/`, applied in order at start-up.
- The Docker setup keeps the data in the `tpt-ruc-data` volume. Back it up before an upgrade. [docs/guides/reverse-proxy-and-backups.md](../../docs/guides/reverse-proxy-and-backups.md) covers backups and restore testing.
- Serve the API only over HTTPS through a reverse proxy when it is reachable from outside the machine. The key is sent with every request.

## Development

```sh
cargo test -p tpt-ruc-backend
cargo clippy -p tpt-ruc-backend --all-targets -- -D warnings
```

The layout:

```
src/main.rs         Entry point, including the healthcheck subcommand
src/lib.rs          Routing, CORS and request logging
src/config.rs       Environment configuration
src/server.rs       Hyper HTTP/1 server
src/db.rs           SQLite access and migrations
src/auth.rs         Principals and roles
src/sessions.rs     Session tokens
src/passwords.rs    Password hashing (Argon2)
src/trips.rs        Trip ingest and re-validation
src/reports.rs      CSV reports and PDF (pdf.rs)
src/eruc.rs         eRUC export and the push provider trait
tests/api.rs        End-to-end API tests
```

The HTTP routing is written in the crate rather than taken from a web framework, which keeps the dependency tree within the project's licence policy.

## Licence

Dual licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
