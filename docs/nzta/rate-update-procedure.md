# Rate table update procedure

Written for: the maintainer who checks NZTA's rates and changes the code. This is the only place where rates change.

Rates are data, not logic. Each rate table is a named, dated constant in `crates/tpt-ruc-core/src/ruc_rates.rs`. A new table is added beside the old one. Older tables are never edited, so trips dated under an old table still give the same answer.

## Rules

1. **Never invent a rate.** Every figure must be copied from an NZTA source, and the source must be recorded.
2. **Never edit an existing table to change a rate.** Add a new table with a new effective date. Trips before that date keep the old table.
3. **Never guess an effective date.** If the page does not state one, record the gap, as `docs/nzta/rates-research.md` does, and do not invent a date.
4. **Ship rate changes as their own commit,** separate from code changes, so the rate history can be read on its own.

## Step 1: check the source

- Open NZTA's "RUC rates and transaction fees" page: <https://www.nzta.govt.nz/vehicles/road-user-charges/ruc-rates-and-transaction-fees>.
- Save a copy of the page as a PDF, and note the date you saved it. Keep the PDF with the commit that changes the rates. Do not commit it if it is large or copyright-restricted; note where it is kept instead.
- Note the date the page says the rates take effect. If the page gives a different date for different vehicle types, record each date.
- Look for any statement that heavy vehicle licences expire after a rate increase. The product models that, so check it against the new table.

## Step 2: record the source in docs

In `docs/nzta/rates-research.md`, add:

- the URL,
- the retrieval date,
- the effective date or dates,
- any figure that could not be confirmed, and why.

## Step 3: add the table

In `ruc_rates.rs`, follow the existing pattern:

1. Add a rules constant, such as `RULES_YYYY_MM_DD`, with one `Rule` per vehicle type and weight band. Keep each type's bands in ascending order, as the `RateTable` documentation requires.
2. Add a table constant, such as `NZTA_YYYY_MM_DD`, with:
   - `version`: a string that names the source and its retrieval date. Every liability figure shows this string.
   - `effective`: the date the table takes effect, as a `RateDate`.
   - `rules`: the rules constant.
   - `gst_basis_points`: `1500` for 15% GST.
3. Add the table to `NZTA_SCHEDULE`, after the existing tables. The schedule must stay in date order.
4. If additional overweight licences, transaction fees or the electric vehicle start date change, update `ADDITIONAL_LICENCES`, `admin_fee_incl_gst_micros` or `EV_LIABLE_FROM` the same way. Do not change a value that the new table does not mention.

## Step 4: check the tests

Run:

```sh
cargo test -p tpt-ruc-core
cargo test -p tpt-ruc-backend
cargo test -p tpt-ruc-ffi-common
```

The `ruc_rates` tests check the transcribed values and the weight-band rules. Add a test for every figure in the new table that a user could check against NZTA, such as one known charge per type. Compare the charge with the NZTA source, not with the code's own output.

If a test fails, decide which is wrong: the code, or the source. Do not change a test to match the code without checking the source first.

## Step 5: check the backend and apps

- `crates/tpt-ruc-backend` stores the `rate_version` with every trip. Check that old trips keep their old version.
- The web app and the mobile bindings use the same core, so they pick up the new table when rebuilt. Rebuild and retest them.

## Step 6: update the records

- Add a `CHANGELOG.md` entry under the release that ships the change. Name the effective date.
- Update the open items in `docs/nzta/rates-research.md`.
- Update `todo.md`: the rate-table item stays open until the next check is scheduled.

## Step 7: schedule the next check

NZTA can change rates at any time. Record the date of the next check in `docs/nzta/rates-research.md`. The project does not monitor the page automatically.

## Checklist for each change

- [ ] Source URL and retrieval date recorded
- [ ] Effective date recorded, or the gap noted
- [ ] New table added, no old table edited
- [ ] Table added to `NZTA_SCHEDULE` in date order
- [ ] Tests added for each checked figure, and all tests pass
- [ ] `rate_version` string names the source and date
- [ ] Changelog and research notes updated
- [ ] Rate change committed on its own
