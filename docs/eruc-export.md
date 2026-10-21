# eRUC export format

Written for: the owner of a fleet, and the engineer who writes an adapter for an eRUC provider.

This describes the generic eRUC export that the server builds from stored trips. Two formats are provided: `generic-json` and `generic-csv`. A provider adapter maps the same records to its own format. The provider formats are not known yet, so no adapter exists. The export does not send anything to a provider. Pushing is a separate job, not built yet.

## Getting an export

Managers and admins can call the endpoint. Drivers cannot, because it covers every driver's trips.

```sh
curl -H "Authorization: Bearer YOUR-KEY" \
  "http://localhost:8080/v1/eruc/export?from=2026-10-01&to=2026-10-31&provider=generic-json"
```

| Query parameter | Default | Meaning |
|---|---|---|
| `from` | open | First trip date to include, `YYYY-MM-DD`, inclusive. |
| `to` | open | Last trip date to include, `YYYY-MM-DD`, inclusive. |
| `provider` | `generic-json` | `generic-json` or `generic-csv`. Any other name is a `400`. |

The reply is a download named `eruc-<provider>-<from>-<to>.<json|csv>`. An open end appears in the name as `0000-01-01` or `9999-12-31`.

## Trips included

Every stored trip whose date falls in the range, oldest first, then by trip ID. Trips are charged by the server, so the amounts are the server's re-validated figures, not amounts sent by a device.

## JSON layout (`generic-json`)

```json
{
  "format": "tpt-ruc-eruc",
  "version": 1,
  "from": "2026-10-01",
  "to": "2026-10-31",
  "records": [
    {
      "trip_id": "trip-0001",
      "trip_date": "2026-10-01",
      "vehicle_id": null,
      "vehicle_type": "1",
      "gross_kg": 3000,
      "electric": false,
      "distance_km": "12.345",
      "rate_version": "2026-08-06",
      "charge_excl_gst_nzd": "66.09",
      "gst_nzd": "9.91",
      "charge_incl_gst_nzd": "76.00"
    }
  ]
}
```

| Field | Type | Meaning |
|---|---|---|
| `format` | string | Always `tpt-ruc-eruc`. |
| `version` | integer | Layout version. Currently `1`. It changes only when a field changes meaning. |
| `from`, `to` | string | The date range the export covers, inclusive. |
| `records` | array | One object per trip. |
| `trip_id` | string | The trip's ID as the device or app recorded it. |
| `trip_date` | string | `YYYY-MM-DD`. |
| `vehicle_id` | string or null | The registered vehicle, or `null` for a trip with an inline vehicle. |
| `vehicle_type` | string | The NZTA RUC type code the trip was charged under. |
| `gross_kg` | integer | Gross vehicle weight, in kilograms. |
| `electric` | boolean | Whether the vehicle is electric. |
| `distance_km` | string | Billed distance, in kilometres, with three decimal places. |
| `rate_version` | string | The rate table the charge was calculated with. |
| `charge_excl_gst_nzd` | string | The charge before GST, in NZD, two decimal places. |
| `gst_nzd` | string | The GST in the charge, in NZD. |
| `charge_incl_gst_nzd` | string | The charge including GST, in NZD. |

The amounts in the examples are illustrative. Money and distance are text, not JSON numbers. Text avoids rounding errors when a provider parses them. Amounts round half up to the cent, once, from the exact total, the same as the NZTA CSV reports.

The recording user is never included. A provider receives the trip and the vehicle, not the driver's identity.

## CSV layout (`generic-csv`)

The first line is the header, then one line per trip. Lines end in CRLF.

```
trip_id,trip_date,vehicle_id,vehicle_type,gross_kg,electric,distance_km,rate_version,charge_excl_gst_nzd,gst_nzd,charge_incl_gst_nzd
trip-0001,2026-10-01,,1,3000,false,12.345,2026-08-06,66.09,9.91,76.00
```

Columns match the JSON fields in the same order. A missing `vehicle_id` is an empty field. Text fields are quoted when they contain a comma, quote or line break. Text that starts with `=`, `+`, `-`, `@`, a tab or a carriage return gets an apostrophe in front, so a spreadsheet does not run it as a formula.

## Versioning

- A new field added at the end, with the meaning of the others unchanged, does not change `version`.
- Renaming, removing or changing the meaning of a field bumps `version`.

## Push jobs (optional)

Pushing sends an export to a provider in the background. It is off unless the owner sets `TPT_RUC_ERUC_PROVIDER`. The only name the server knows today is `dry-run`, a placeholder that sends nothing and records that in the audit log. It exists to test the queue. A real provider needs its own adapter, and no provider adapter exists yet.

| Endpoint | Who | What |
|---|---|---|
| `POST /v1/eruc/jobs` | manager, admin | Body `{"provider":"dry-run","from":"2026-10-01","to":"2026-10-31"}`. Returns `202` with the queued job. `503` when no provider is configured. `400` for another provider name or bad dates. |
| `GET /v1/eruc/jobs` | manager, admin | The 100 most recent jobs, newest first. |
| `GET /v1/eruc/jobs/{id}` | manager, admin | One job, with its audit log. |

Each job goes through these states: `queued`, `running`, then `succeeded` or back to `queued`. A failure the provider marks retryable waits 60 seconds, then 2, 4, 8 minutes, and so on, up to 6 hours. After 5 attempts the job becomes `failed`. A permanent failure fails at once. A job that was running when the server stopped is requeued on the next start. Every change is written to the audit log, so the history stays after the job finishes. The worker checks for due jobs once a minute.

The batch is built from the trips stored at the moment of each attempt, not when the job was queued.

## Adapters

There are two traits in `crates/tpt-ruc-backend/src/eruc.rs`. `ErucProvider` builds a file payload, and the generic formats implement it. `PushProvider` sends a batch, and returns `Ok` with a note or a `PushError` that is either `Retryable` or `Permanent`. A new provider implements `PushProvider`, is added to `push_provider_named`, and is covered by the queue's retry rules. Real adapters wait for the provider API documents.
