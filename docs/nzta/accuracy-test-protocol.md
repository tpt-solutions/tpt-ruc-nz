# Accuracy test protocol

Written for: the person who runs the distance tests, and the owner who reviews them.

> **This is a protocol, not results.** No test has been run. The thresholds below are placeholders until the accuracy requirement is read from NZTA's own text (see `section43-gap-analysis.md`). Do not report a pass or fail against them.

## Purpose

To measure how close the recorder's distance is to a known true distance, under conditions a driver meets, and to show that the measurement repeats.

## Scope

- The GPS path only: receiver, firmware filters and the core's distance calculation.
- The manual trip entry in the web app is out of scope. It is not a measurement.
- The backend is out of scope, except to confirm that trip summaries are stored and returned unchanged.

## Equipment and records

- The device under test, with its firmware and core version recorded. Use the exact git commit.
- The GNSS antenna and its mounting, recorded with a photograph.
- A reference measurement. Choose one before the tests and record its accuracy:
  - a surveyed course, with its length measured by a method stated in advance, or
  - a reference GNSS receiver with a stated accuracy, or
  - the distance on a calibrated measuring wheel.
- A log of every raw NMEA sentence from the receiver, with the receiver's time. Save the raw log, not only the processed trip.
- A note of the weather, the sky view and the road conditions for each run.

## Test routes

Use at least these routes. Each one must be driven at least the number of times set in the NZTA text, and at least three times here until that text is read.

| Route | What it tests |
|---|---|
| Open motorway or highway, straight, at speed | Accuracy at steady high speed |
| Urban streets with stops, turns and traffic lights | Stationary drift and jumps |
| Route with tall buildings or trees | Signal loss and gaps |
| Route through a tunnel or underpass | Recovery after a long gap |
| Stop-start in a car park | Drift while stationary |

Record the true distance of each route before the first run.

## Procedure

1. Confirm the reference measurement and its accuracy. Record both.
2. Power the device. Wait for a fix, and record the time to first fix.
3. Drive the route. Start and stop the trip at the same marked points each run.
4. Save the raw NMEA log and the trip output the device produced.
5. After each run, record the reference distance, the device distance, and the run's conditions.
6. Repeat until the required number of runs is complete. Do not drop a run without writing down why.

## Bench tests (before any drive)

- **Stationary test.** Leave the device still for at least one hour under open sky. The recorded distance should stay at or near zero. Record the drift.
- **Replay test.** Feed a recorded NMEA log through the core twice. The two results must match exactly. This checks determinism.
- **Filter test.** Feed logs with known bad fixes, such as high HDOP or a sudden jump. Check each is rejected.

## Metrics

For each route, report:

- the number of runs,
- the mean device distance and the mean reference distance,
- the mean error in metres and as a percentage of the reference,
- the largest error for any single run,
- the spread between repeated runs.

Also report the time to first fix, the fraction of time with a valid fix, and the stationary drift.

## Acceptance

**Needs NZTA source.** The acceptance limits are not set in this protocol. Write them in as a single table, from NZTA's text, before the first run. Do not change them after the tests start.

## Records to keep

- The protocol version and the date of each run.
- The firmware, core and receiver versions.
- Every raw log, unedited.
- Every reference measurement and how it was made.
- Every run, including failed ones.

## Reporting

A report must state what was tested, how, with which equipment, and the result against the stated limits. It must include the failed runs. A report that omits runs is not a valid report.

## Status

- [ ] Reference measurement method chosen and its accuracy recorded
- [ ] Acceptance limits taken from NZTA text
- [ ] Bench tests run
- [ ] Route tests run
- [ ] Report written
