# Privacy statement

Written for: anyone who uses tpt-ruc-nz, and anyone deciding whether to trust it with their travel data.

This statement describes what the code does today. Where the code does something it does not yet promise, it says so.

## The short version

- The web app keeps everything in your browser. It makes no network requests.
- The self-hosted server keeps everything on the computer it runs on. TPT Solutions receives nothing.
- The server stores trip summaries (date, vehicle, distance and charge). It does not store GPS points.
- You can export and delete your data at any time.

## The web app (PWA)

The web app stores these in your browser's own storage (IndexedDB) on your device:

- Vehicles: names, type codes, weights, and odometer readings you enter.
- Trips: dates, distances, vehicles and the charge the app works out.
- Device segments: trip segments you import from a file.
- Settings: such as whether you have accepted the Section 43 notice.

It does not send any of this anywhere. The source code contains no network calls to a server. Clearing your browser's site data deletes all of it.

The Backup feature saves all of this to a file you download. That file is not encrypted. Keep it private.

## The self-hosted server

### What is stored

In the SQLite database, in the data folder (`./data` when run without Docker, or the `tpt-ruc-data` Docker volume):

- **Users** (only if you create them): an ID, a display name, a role, the creation time, and a hash of the user's token. The token itself is never stored. A SHA-256 hash is stored in its place, so a copy of the database cannot be used to sign in.
- **Trips:** an ID you choose, the trip date, the vehicle type code and weight, whether the vehicle is electric, the distance, the charge the server calculated, the rate version used, the time the server received the trip, and the ID of the user who recorded it. Trips recorded with the owner key have no user.

The server does not store GPS coordinates, and it does not store the names or times of individual journeys beyond the trip date.

The API key is stored in `api-key` in the same folder, readable only by its owner on Linux and Mac.

### What is logged

The server writes a log line for each request, with:

- the HTTP method (for example `GET`),
- the path (for example `/v1/trips`). The query string is not logged, so the dates in a report request do not appear in the log,
- the response status,
- how long the request took.

Other log lines record:

- a user being deleted, with the user's ID and how many trips were removed,
- retention removing old trips, with how many (only if retention is set),
- trips being deleted, with the number removed and the date range,
- database migrations being applied at startup,
- the API key file being used or generated. A generated key is printed to the log, so the log holds it. See the [self-hosting guide](guides/self-hosting.md).

The code does **not** log client IP addresses or the trip IDs of individual uploads.

### Rate limiting

To limit abuse, the server counts requests per client IP address in memory, for up to 15 minutes. These counts are not written to disk or to the log. The table holds at most 10,000 addresses, and older entries are dropped when it fills.

### Export and deletion

- `GET /v1/export` returns every trip you are allowed to see as a JSON file. A driver gets only their own trips.
- `DELETE /v1/trips?from=...&to=...` deletes trips in a date range. Drivers delete only their own trips.
- `DELETE /v1/users/{id}` deletes a user and every trip they recorded.
- A full wipe is a deletion over the widest range, `from=0000-01-01&to=9999-12-31`.

### What is not done

- **Retention is off by default.** Trips stay until someone deletes them. If you set `TPT_RUC_RETENTION_DAYS`, the server deletes trips older than that many days, for every user, checked every hour. It logs only how many trips it removed.
- **No encryption at rest.** The database file is not encrypted. Protect the computer and its backups.
- **No encryption in transit on its own.** The server speaks plain HTTP. Put it behind HTTPS before any device other than the server's own computer reaches it.
- **No sharing with other people.** Each server belongs to one owner. Organisations and shared fleets are planned.

## Backups

A backup of the server is a copy of the data folder or volume. It contains your trips, users and the API key. Treat it as private.

## Changes

This statement will change as the code changes. When sync to the server is added to the web app, this statement will be updated before that release.
