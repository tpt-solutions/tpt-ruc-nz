# NMEA test fixtures

These files are recordings of GNSS receiver output. The parser tests in
`tests/nmea_fixtures.rs` read every file here and check that each line parses.

## The fixtures in this folder

| File | What it is |
|---|---|
| `synthetic-one-epoch.nmea` | **Synthetic.** Hand-written for this project, not captured from a receiver. It has one epoch with GGA, RMC, VTG and GSA sentences. Its checksums were calculated by script. Use it to check the parser's structure, not a receiver's behaviour. |

## Real-world captures: what we still need

The parser has not yet been checked against real receiver output. Please add
captures from real hardware, for example the u-blox NEO-M8N on the bench
breadboard (see `hardware/README.md`) or a phone or car receiver.

For each capture:

1. Record raw NMEA with no editing: a serial logger, `gpsd` with `gpspipe -r`,
   or the phone's NMEA export. Keep the sentences exactly as the receiver sent them.
2. Record at least five minutes, and include at least one of these:
   - a cold start, where the first fixes have no position or poor HDOP;
   - a stop at a traffic light or in a car park, to show stationary drift;
   - a tunnel or an urban canyon, to show signal loss and recovery;
   - a multi-constellation receiver (GP, GN, GL, GA or BD talkers).
3. Save the file as `<receiver>-<situation>-<yyyy-mm-dd>.nmea`, for example
   `neo-m8n-stationary-2026-11-02.nmea`. Use lowercase letters and hyphens only.
4. Name the receiver, its rate and its antenna in a companion note, such as
   `neo-m8n-stationary-2026-11-02.md`. The note is what tells a future reader
   what the file shows.
5. Remove any personal location. If the capture is from a home address, shift
   the coordinates by a fixed offset and say so in the note. Never commit a raw
   home track.

## Adding a file

1. Copy the `.nmea` file into this folder.
2. Add an `include_str!` constant and a test in `tests/nmea_fixtures.rs`, in the
   same form as the synthetic one.
3. Run `cargo test -p tpt-ruc-core`. Every line must parse. If one fails,
   check the line first. A genuine receiver output the parser rejects is a
   bug to fix, not a line to delete.

Lines that fail the checksum on purpose, to test rejection, belong in a test
string, not in these files.
