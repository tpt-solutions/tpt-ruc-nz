# Self-hosting the backend

Written for: the person who will run the server on their own computer, NAS or Raspberry Pi.

The backend stores trip summaries, users and reports. You can run it on your own machine. Nothing is sent to TPT Solutions.

The web app (PWA) can upload its trips to the server from its sync panel. See [PWA guide](pwa.md). You can also call the API directly, for example with `curl`.

## What you need

- Docker Desktop (Windows or Mac), or Docker Engine with the Compose plugin (Linux).
- The project folder. Download it with git, or as a ZIP from the project page.

You do not need to install Rust.

## Start the server

From the project folder, run:

```sh
docker compose up -d
docker compose logs tpt-ruc
```

On the first start the log shows your API key:

```
No API key was set, so one was generated and saved to:
  /data/api-key

Use it as the Bearer token:
  Authorization: Bearer 3f9c...
```

Copy that key somewhere safe. Every request needs it.

Check that the server answers:

```sh
curl -H "Authorization: Bearer YOUR-KEY" http://localhost:8080/v1/trips
```

You should see `{"limit":100,"offset":0,"trips":[]}`. On Windows PowerShell, use `curl.exe`. The `curl` name there runs a different program.

## Everyday commands

| To do this | Run |
|---|---|
| Start the server | `docker compose up -d` |
| Stop it, keeping your data | `docker compose down` |
| See the log, including the key | `docker compose logs tpt-ruc` |
| Update after pulling new code | `git pull`, then `docker compose up -d --build` |
| Change a setting | Edit `.env` (copy `.env.example` first), then `docker compose up -d` |

## Settings

Put settings in a file called `.env` next to `docker-compose.yml`. Every setting is optional.

| Setting | Default | What it does |
|---|---|---|
| `TPT_RUC_API_KEY` | generated on first start | Your key. Use at least 32 characters. Leave blank to keep the generated key. |
| `TPT_RUC_CORS_ORIGINS` | none | Websites allowed to call the API from a browser, separated by commas. Leave blank unless you host a web app elsewhere. |

The server also reads these variables. The Docker image already sets them, so you only need them if you run the program without Docker:

| Variable | Default without Docker | Default in Docker | Meaning |
|---|---|---|---|
| `TPT_RUC_BIND` | `127.0.0.1:8080` | `0.0.0.0:8080` | Address and port to listen on. |
| `TPT_RUC_DATABASE_URL` | `sqlite://data/tpt-ruc.db` | `sqlite:///data/tpt-ruc.db` | Where the database file lives. |
| `RUST_LOG` | `info` | `info` | How much to log: `error`, `warn`, `info`, `debug`. |
| `TPT_RUC_RETENTION_DAYS` | off | off | Optional. Delete trips older than this many days, for every user. Checked every hour. |
| `TPT_RUC_ERUC_PROVIDER` | off | off | Optional. Name of an eRUC push provider. The only one so far is `dry-run`, which sends nothing. See [eRUC export format](../eruc-export.md). |
| `TPT_RUC_PWA_DIR` | off | off | Optional. A folder with the built web app (`pwa/out`). When set, the server serves the app on its own address. |
| `TPT_RUC_ALLOW_RAW_GPS` | off | off | Optional. Set to `true` to let users upload raw GPS points to `POST /v1/gps/points`. Off means summaries only, and the server refuses raw points. |
| `TPT_RUC_CONFIG` | none | none | Optional. Path to a TOML file holding the same settings, keyed by their variable names. An environment variable overrides the file. |

## Users and roles

The owner key has full access. To let other people record trips, create a user for each one. You need the owner key for this.

```sh
curl -X POST -H "Authorization: Bearer YOUR-KEY" -H "Content-Type: application/json" \
  -d '{"name":"Aroha","role":"driver"}' http://localhost:8080/v1/users
```

The reply contains that user's token. It is shown once, so copy it then. The roles are:

| Role | Can do |
|---|---|
| `driver` | Record trips, and see, export and delete only their own trips. |
| `manager` | See every trip, and run the CSV reports. |
| `admin` | Everything a manager can do, and create, list and delete users. |

Deleting a user also deletes every trip they recorded. The reply says how many trips went:

```sh
curl -X DELETE -H "Authorization: Bearer YOUR-KEY" http://localhost:8080/v1/users/USER-ID
```

## Reports and export

- `GET /v1/reports/trips.csv`: one row per trip, for audit. Manager or admin only.
- `GET /v1/reports/monthly.csv`: totals per month and vehicle type. Manager or admin only.
- `GET /v1/reports/monthly.pdf`: the same monthly totals as a PDF. Manager or admin only.
- `GET /v1/eruc/export`: trips in a generic eRUC format, as JSON or CSV. Manager or admin only. See [eRUC export format](../eruc-export.md).
- `GET /v1/export`: every trip you may see, as JSON.

The report URLs accept `from` and `to` dates, written as `2026-10-01`, to limit the period.

## Backups

All data is in one place: a Docker volume whose name ends in `tpt-ruc-data`. It holds the database and the saved API key.

Stop the server, copy the volume to an archive, then start the server again. [HTTPS, backup routines and restore tests](reverse-proxy-and-backups.md) give the exact commands, and explain how to find the volume's full name.

Keep the backup safe. It contains your trips and your API key. Store it somewhere private, and test that you can restore it.

## Keep it private

- The compose file listens only on `127.0.0.1`. Only this computer can reach the server.
- Anyone who can read `docker compose logs` can read the API key. Do not share that log.
- To reach the server from another device, you must put it behind HTTPS first. A reverse proxy such as Caddy can do that, and [the guide](reverse-proxy-and-backups.md) shows how. Without HTTPS, the key travels in plain text on your network.
- The server limits each client to 120 requests a minute. After 10 failed key checks within 15 minutes, a client is blocked for those 15 minutes.

## Upgrading

Back up first. Then pull the new code and rebuild. The database is updated automatically when the server starts, and each change is recorded so it runs only once.

## Running without Docker

Install [Rust](https://rustup.rs), then from the project folder run:

```sh
cargo run -p tpt-ruc-backend
```

The first start creates `./data/`, with the database and the saved key. Stop the server with Ctrl+C.

## Health checks

- `GET /healthz`: the server is running. No key needed.
- `GET /readyz`: the server can reach its database. No key needed.
- `tpt-ruc-backend healthcheck`: the same check from the command line. It exits 0 when healthy. The Docker image runs it automatically.

## If something goes wrong

- **Port 8080 is already in use.** Another program is using the port. Stop it, or change `"127.0.0.1:8080:8080"` in `docker-compose.yml` to another port such as `"127.0.0.1:8081:8080"`.
- **`401 unauthorized`.** The key is wrong, or the header is missing. The header must read `Authorization: Bearer YOUR-KEY`, with a space after `Bearer`.
- **`429 too many requests`.** You have sent too many requests, or too many wrong keys. Wait for the time in the `Retry-After` header.
- **The key file is shorter than 32 characters.** The server refuses to start, so it does not run with a weak key. Delete the file to generate a new one, or set `TPT_RUC_API_KEY` to a longer key.
