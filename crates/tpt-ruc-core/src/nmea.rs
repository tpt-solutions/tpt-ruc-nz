// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! NMEA 0183 sentence parsing for GNSS receivers.
//!
//! Supported sentences: GGA (fix), RMC (recommended minimum), VTG (track and
//! speed) and GSA (DOP and fix type). Other sentences parse to
//! [`Sentence::Unsupported`] once their checksum is verified.
//!
//! Talker IDs (GP, GN, GL, GA, BD, ...) are accepted and recorded. The parser
//! works on a single sentence with no allocation. Feed it one line at a time.

use core::fmt;

use crate::coord::Coordinate;

/// Maximum number of comma-separated fields read from one sentence. Real
/// NMEA sentences use at most 19 (GSA), so extra fields are ignored.
const MAX_FIELDS: usize = 24;

/// Why a sentence could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// The line does not start with `$`.
    MissingStart,
    /// The line has no `*` checksum delimiter.
    MissingChecksum,
    /// The checksum is not two hex digits, or the address field is malformed.
    InvalidChecksum,
    /// The computed XOR checksum does not match the transmitted one.
    ChecksumMismatch {
        /// Checksum computed from the payload.
        expected: u8,
        /// Checksum transmitted in the sentence.
        found: u8,
    },
    /// The sentence has fewer fields than required. Holds the field name.
    MissingField(&'static str),
    /// A field is present but its value is not valid. Holds the field name.
    InvalidField(&'static str),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStart => f.write_str("sentence does not start with '$'"),
            Self::MissingChecksum => f.write_str("sentence has no '*' checksum delimiter"),
            Self::InvalidChecksum => f.write_str("checksum or address field is malformed"),
            Self::ChecksumMismatch { expected, found } => {
                write!(
                    f,
                    "checksum mismatch: expected {expected:02X}, found {found:02X}"
                )
            }
            Self::MissingField(name) => write!(f, "missing field: {name}"),
            Self::InvalidField(name) => write!(f, "invalid field: {name}"),
        }
    }
}

/// Talker identifier from the start of the address field, for example `GN`
/// for multi-constellation or `GP` for GPS only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Talker(pub [u8; 2]);

impl Talker {
    /// Returns the two-letter talker code.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or("??")
    }
}

/// GNSS fix quality, from the GGA quality indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FixQuality {
    /// No fix (indicator 0, or empty).
    NoFix,
    /// Standard GPS fix (1).
    Gps,
    /// Differential GPS fix (2).
    Dgps,
    /// Precise positioning service (3).
    Pps,
    /// RTK fixed (4).
    RtkFixed,
    /// RTK float (5).
    RtkFloat,
    /// Dead-reckoning estimate (6).
    Estimated,
    /// Manual input (7).
    Manual,
    /// Simulation mode (8).
    Simulation,
}

impl FixQuality {
    fn from_field(value: &str) -> Result<Self, ParseError> {
        if value.is_empty() {
            return Ok(Self::NoFix);
        }
        match value {
            "0" => Ok(Self::NoFix),
            "1" => Ok(Self::Gps),
            "2" => Ok(Self::Dgps),
            "3" => Ok(Self::Pps),
            "4" => Ok(Self::RtkFixed),
            "5" => Ok(Self::RtkFloat),
            "6" => Ok(Self::Estimated),
            "7" => Ok(Self::Manual),
            "8" => Ok(Self::Simulation),
            _ => Err(ParseError::InvalidField("fix quality")),
        }
    }

    /// Returns true for any fix that carries a usable position.
    pub fn has_position(&self) -> bool {
        !matches!(self, Self::NoFix | Self::Simulation)
    }
}

/// Dimension of the fix, from the GSA mode indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixType {
    /// No fix.
    NoFix,
    /// 2D fix (latitude and longitude only).
    Fix2D,
    /// 3D fix (latitude, longitude and altitude).
    Fix3D,
}

/// UTC time of day from an NMEA `hhmmss[.sss]` field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UtcTime {
    /// Hour, 0..=23.
    pub hour: u8,
    /// Minute, 0..=59.
    pub minute: u8,
    /// Second, 0.0..=60.999 (60 allowed for leap seconds).
    pub second: f64,
}

/// Calendar date from an NMEA `ddmmyy` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    /// Day of month, 1..=31.
    pub day: u8,
    /// Month, 1..=12.
    pub month: u8,
    /// Full year. NMEA carries two digits, read as 19xx for 80..=99 and 20xx for 00..=79.
    pub year: u16,
}

/// GGA: time, position, fix quality and accuracy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gga {
    /// Talker that produced the sentence.
    pub talker: Talker,
    /// UTC time, if reported.
    pub time: Option<UtcTime>,
    /// Position, if the receiver has one.
    pub position: Option<Coordinate>,
    /// Fix quality.
    pub fix: FixQuality,
    /// Number of satellites in use.
    pub satellites: Option<u8>,
    /// Horizontal dilution of precision.
    pub hdop: Option<f64>,
    /// Altitude above mean sea level, in metres.
    pub altitude_m: Option<f64>,
}

/// RMC: time, position, speed, course and date.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rmc {
    /// Talker that produced the sentence.
    pub talker: Talker,
    /// UTC time, if reported.
    pub time: Option<UtcTime>,
    /// True when the receiver marks the data as valid (status `A`).
    pub valid: bool,
    /// Position, if the receiver has one.
    pub position: Option<Coordinate>,
    /// Speed over ground, in knots.
    pub speed_knots: Option<f64>,
    /// Course over ground, in degrees true.
    pub course_deg: Option<f64>,
    /// Date, if reported.
    pub date: Option<Date>,
}

/// VTG: course and speed over ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vtg {
    /// Talker that produced the sentence.
    pub talker: Talker,
    /// Course over ground, in degrees true.
    pub course_true_deg: Option<f64>,
    /// Speed over ground, in knots.
    pub speed_knots: Option<f64>,
    /// Speed over ground, in km/h.
    pub speed_kmh: Option<f64>,
}

/// GSA: fix dimension and dilution of precision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gsa {
    /// Talker that produced the sentence.
    pub talker: Talker,
    /// Fix dimension.
    pub fix_type: FixType,
    /// Position dilution of precision.
    pub pdop: Option<f64>,
    /// Horizontal dilution of precision.
    pub hdop: Option<f64>,
    /// Vertical dilution of precision.
    pub vdop: Option<f64>,
}

/// A parsed NMEA sentence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sentence {
    /// GGA fix data.
    Gga(Gga),
    /// RMC recommended minimum data.
    Rmc(Rmc),
    /// VTG course and speed.
    Vtg(Vtg),
    /// GSA DOP and active satellites.
    Gsa(Gsa),
    /// A valid sentence type this parser does not decode.
    Unsupported,
}

/// Parses one NMEA sentence, such as `$GPGGA,...*47`.
///
/// A trailing CR or LF is ignored. The checksum is verified before any field
/// is read, so corrupt sentences are rejected whatever their type.
pub fn parse_sentence(line: &str) -> Result<Sentence, ParseError> {
    let line = line.trim_end_matches(['\r', '\n']);
    let body = line.strip_prefix('$').ok_or(ParseError::MissingStart)?;
    let (payload, checksum) = body.rsplit_once('*').ok_or(ParseError::MissingChecksum)?;

    verify_checksum(payload, checksum)?;

    let mut buf: [&str; MAX_FIELDS] = [""; MAX_FIELDS];
    let mut count = 0;
    for field in payload.split(',').take(MAX_FIELDS) {
        buf[count] = field;
        count += 1;
    }
    let fields = &buf[..count];

    let address = fields[0];
    if address.len() != 5 || !address.is_ascii() {
        return Err(ParseError::InvalidField("address"));
    }
    let raw = address.as_bytes();
    let talker = Talker([raw[0], raw[1]]);
    let fields = &fields[1..];

    match &address[2..] {
        "GGA" => parse_gga(talker, fields),
        "RMC" => parse_rmc(talker, fields),
        "VTG" => parse_vtg(talker, fields),
        "GSA" => parse_gsa(talker, fields),
        _ => Ok(Sentence::Unsupported),
    }
}

fn verify_checksum(payload: &str, checksum: &str) -> Result<(), ParseError> {
    if checksum.len() != 2 {
        return Err(ParseError::InvalidChecksum);
    }
    let found = u8::from_str_radix(checksum, 16).map_err(|_| ParseError::InvalidChecksum)?;
    let expected = payload.bytes().fold(0u8, |acc, b| acc ^ b);
    if expected == found {
        Ok(())
    } else {
        Err(ParseError::ChecksumMismatch { expected, found })
    }
}

fn field<'a>(fields: &[&'a str], index: usize, name: &'static str) -> Result<&'a str, ParseError> {
    fields
        .get(index)
        .copied()
        .ok_or(ParseError::MissingField(name))
}

fn opt_f64(value: &str, name: &'static str) -> Result<Option<f64>, ParseError> {
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<f64>()
        .map(Some)
        .map_err(|_| ParseError::InvalidField(name))
}

fn opt_u8(value: &str, name: &'static str) -> Result<Option<u8>, ParseError> {
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<u8>()
        .map(Some)
        .map_err(|_| ParseError::InvalidField(name))
}

fn digits(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse::<u32>().ok()
}

fn parse_time(value: &str) -> Result<Option<UtcTime>, ParseError> {
    const NAME: &str = "time";
    if value.is_empty() {
        return Ok(None);
    }
    let bad = ParseError::InvalidField("time");
    let hour = digits(value.get(0..2).ok_or(bad)?).ok_or(bad)?;
    let minute = digits(value.get(2..4).ok_or(bad)?).ok_or(bad)?;
    let second = value
        .get(4..)
        .ok_or(bad)?
        .parse::<f64>()
        .map_err(|_| ParseError::InvalidField(NAME))?;
    if hour > 23 || minute > 59 || !(0.0..61.0).contains(&second) {
        return Err(bad);
    }
    Ok(Some(UtcTime {
        hour: hour as u8,
        minute: minute as u8,
        second,
    }))
}

fn parse_date(value: &str) -> Result<Option<Date>, ParseError> {
    if value.is_empty() {
        return Ok(None);
    }
    let bad = ParseError::InvalidField("date");
    if value.len() != 6 {
        return Err(bad);
    }
    let day = digits(value.get(0..2).ok_or(bad)?).ok_or(bad)?;
    let month = digits(value.get(2..4).ok_or(bad)?).ok_or(bad)?;
    let yy = digits(value.get(4..6).ok_or(bad)?).ok_or(bad)?;
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) {
        return Err(bad);
    }
    // Two-digit year pivot at 80: 94 is 1994, 26 is 2026.
    let year = if yy >= 80 { 1900 + yy } else { 2000 + yy } as u16;
    Ok(Some(Date {
        day: day as u8,
        month: month as u8,
        year,
    }))
}

#[derive(Clone, Copy)]
enum Axis {
    Lat,
    Lon,
}

/// Parses an NMEA `ddmm.mmmm` (latitude) or `dddmm.mmmm` (longitude) value
/// with its hemisphere letter, returning signed decimal degrees.
fn parse_angle(value: &str, hemisphere: &str, axis: Axis) -> Result<Option<f64>, ParseError> {
    if value.is_empty() {
        return Ok(None);
    }
    let (deg_len, name, sign) = match (axis, hemisphere) {
        (Axis::Lat, "N") => (2, "latitude", 1.0),
        (Axis::Lat, "S") => (2, "latitude", -1.0),
        (Axis::Lon, "E") => (3, "longitude", 1.0),
        (Axis::Lon, "W") => (3, "longitude", -1.0),
        (Axis::Lat, _) => return Err(ParseError::InvalidField("latitude hemisphere")),
        (Axis::Lon, _) => return Err(ParseError::InvalidField("longitude hemisphere")),
    };
    let bad = ParseError::InvalidField(name);

    // Minutes always have exactly two integer digits before the decimal point.
    let dot = value.find('.').unwrap_or(value.len());
    if dot != deg_len + 2 {
        return Err(bad);
    }
    let degrees = digits(value.get(..deg_len).ok_or(bad)?).ok_or(bad)?;
    let minutes = value
        .get(deg_len..)
        .ok_or(bad)?
        .parse::<f64>()
        .map_err(|_| bad)?;
    if !(0.0..60.0).contains(&minutes) {
        return Err(bad);
    }
    Ok(Some(sign * (degrees as f64 + minutes / 60.0)))
}

fn parse_position(
    lat: &str,
    lat_hemi: &str,
    lon: &str,
    lon_hemi: &str,
) -> Result<Option<Coordinate>, ParseError> {
    let lat = parse_angle(lat, lat_hemi, Axis::Lat)?;
    let lon = parse_angle(lon, lon_hemi, Axis::Lon)?;
    match (lat, lon) {
        (Some(lat), Some(lon)) => Coordinate::new(lat, lon)
            .map(Some)
            .map_err(|_| ParseError::InvalidField("position")),
        _ => Ok(None),
    }
}

fn parse_gga(talker: Talker, f: &[&str]) -> Result<Sentence, ParseError> {
    let time = parse_time(field(f, 0, "time")?)?;
    let position = parse_position(
        field(f, 1, "latitude")?,
        field(f, 2, "latitude hemisphere")?,
        field(f, 3, "longitude")?,
        field(f, 4, "longitude hemisphere")?,
    )?;
    let fix = FixQuality::from_field(field(f, 5, "fix quality")?)?;
    let satellites = opt_u8(field(f, 6, "satellites")?, "satellites")?;
    let hdop = opt_f64(field(f, 7, "hdop")?, "hdop")?;
    let altitude_m = opt_f64(field(f, 8, "altitude")?, "altitude")?;

    Ok(Sentence::Gga(Gga {
        talker,
        time,
        position,
        fix,
        satellites,
        hdop,
        altitude_m,
    }))
}

fn parse_rmc(talker: Talker, f: &[&str]) -> Result<Sentence, ParseError> {
    let time = parse_time(field(f, 0, "time")?)?;
    let valid = match field(f, 1, "status")? {
        "A" => true,
        "V" | "" => false,
        _ => return Err(ParseError::InvalidField("status")),
    };
    let position = parse_position(
        field(f, 2, "latitude")?,
        field(f, 3, "latitude hemisphere")?,
        field(f, 4, "longitude")?,
        field(f, 5, "longitude hemisphere")?,
    )?;
    let speed_knots = opt_f64(field(f, 6, "speed")?, "speed")?;
    let course_deg = opt_f64(field(f, 7, "course")?, "course")?;
    let date = parse_date(field(f, 8, "date")?)?;

    Ok(Sentence::Rmc(Rmc {
        talker,
        time,
        valid,
        position,
        speed_knots,
        course_deg,
        date,
    }))
}

fn parse_vtg(talker: Talker, f: &[&str]) -> Result<Sentence, ParseError> {
    let course_true_deg = opt_f64(field(f, 0, "course")?, "course")?;
    let speed_knots = opt_f64(field(f, 4, "speed knots")?, "speed knots")?;
    let speed_kmh = opt_f64(field(f, 6, "speed km/h")?, "speed km/h")?;

    Ok(Sentence::Vtg(Vtg {
        talker,
        course_true_deg,
        speed_knots,
        speed_kmh,
    }))
}

fn parse_gsa(talker: Talker, f: &[&str]) -> Result<Sentence, ParseError> {
    let fix_type = match field(f, 1, "fix type")? {
        "" | "1" => FixType::NoFix,
        "2" => FixType::Fix2D,
        "3" => FixType::Fix3D,
        _ => return Err(ParseError::InvalidField("fix type")),
    };
    // Fields 2..=13 are the satellite PRNs in use. They are not stored.
    let pdop = opt_f64(field(f, 14, "pdop")?, "pdop")?;
    let hdop = opt_f64(field(f, 15, "hdop")?, "hdop")?;
    let vdop = opt_f64(field(f, 16, "vdop")?, "vdop")?;

    Ok(Sentence::Gsa(Gsa {
        talker,
        fix_type,
        pdop,
        hdop,
        vdop,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Standard example sentences from the NMEA 0183 literature.
    const GGA: &str = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47";
    const RMC: &str = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A";
    const VTG: &str = "$GPVTG,054.7,T,034.4,M,005.5,N,010.2,K*48";
    const GSA: &str = "$GPGSA,A,3,04,05,,09,12,,,24,,,,,2.5,1.3,2.1*39";

    fn compute_checksum(payload: &str) -> u8 {
        payload.bytes().fold(0, |acc, b| acc ^ b)
    }

    fn with_checksum(payload: &str) -> String {
        format!("${payload}*{:02X}", compute_checksum(payload))
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn parses_gga() {
        let Sentence::Gga(g) = parse_sentence(GGA).unwrap() else {
            panic!("expected GGA");
        };
        assert_eq!(g.talker.as_str(), "GP");
        assert_eq!(
            g.time,
            Some(UtcTime {
                hour: 12,
                minute: 35,
                second: 19.0
            })
        );
        let pos = g.position.unwrap();
        assert!(approx(pos.lat.degrees(), 48.1173));
        assert!(approx(pos.lon.degrees(), 11.516_666_67));
        assert_eq!(g.fix, FixQuality::Gps);
        assert_eq!(g.satellites, Some(8));
        assert_eq!(g.hdop, Some(0.9));
        assert_eq!(g.altitude_m, Some(545.4));
    }

    #[test]
    fn parses_rmc() {
        let Sentence::Rmc(r) = parse_sentence(RMC).unwrap() else {
            panic!("expected RMC");
        };
        assert!(r.valid);
        assert_eq!(r.speed_knots, Some(22.4));
        assert_eq!(r.course_deg, Some(84.4));
        assert_eq!(
            r.date,
            Some(Date {
                day: 23,
                month: 3,
                year: 1994
            })
        );
        assert!(r.position.is_some());
    }

    #[test]
    fn rmc_void_status_is_not_valid() {
        let s = with_checksum("GPRMC,123519,V,,,,,,,230394,,");
        let Sentence::Rmc(r) = parse_sentence(&s).unwrap() else {
            panic!("expected RMC");
        };
        assert!(!r.valid);
        assert_eq!(r.position, None);
        assert_eq!(r.speed_knots, None);
    }

    #[test]
    fn parses_vtg() {
        let Sentence::Vtg(v) = parse_sentence(VTG).unwrap() else {
            panic!("expected VTG");
        };
        assert_eq!(v.course_true_deg, Some(54.7));
        assert_eq!(v.speed_knots, Some(5.5));
        assert_eq!(v.speed_kmh, Some(10.2));
    }

    #[test]
    fn parses_gsa() {
        let Sentence::Gsa(g) = parse_sentence(GSA).unwrap() else {
            panic!("expected GSA");
        };
        assert_eq!(g.fix_type, FixType::Fix3D);
        assert_eq!(g.pdop, Some(2.5));
        assert_eq!(g.hdop, Some(1.3));
        assert_eq!(g.vdop, Some(2.1));
    }

    #[test]
    fn accepts_crlf_terminator() {
        let line = format!("{GGA}\r\n");
        assert!(parse_sentence(&line).is_ok());
    }

    #[test]
    fn multi_constellation_talkers() {
        let s = with_checksum("GNRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,,");
        let Sentence::Rmc(r) = parse_sentence(&s).unwrap() else {
            panic!("expected RMC");
        };
        assert_eq!(r.talker.as_str(), "GN");
    }

    #[test]
    fn southern_and_western_hemispheres_are_negative() {
        // Wellington: 41 deg 17.19 min S, 174 deg 46.57 min E.
        let s = with_checksum("GPGGA,000000,4117.190,S,17446.570,E,1,08,1.0,10.0,M,0.0,M,,");
        let Sentence::Gga(g) = parse_sentence(&s).unwrap() else {
            panic!("expected GGA");
        };
        let pos = g.position.unwrap();
        assert!(pos.lat.degrees() < 0.0);
        assert!(approx(pos.lat.degrees(), -41.286_5));
        assert!(approx(pos.lon.degrees(), 174.776_166_67));

        let w = with_checksum("GPGGA,000000,4117.190,S,01746.570,W,1,08,1.0,10.0,M,0.0,M,,");
        let Sentence::Gga(g) = parse_sentence(&w).unwrap() else {
            panic!("expected GGA");
        };
        assert!(g.position.unwrap().lon.degrees() < 0.0);
    }

    #[test]
    fn empty_gga_fields_are_none_and_no_fix() {
        let s = with_checksum("GPGGA,,,,,,0,,,,,,,,");
        let Sentence::Gga(g) = parse_sentence(&s).unwrap() else {
            panic!("expected GGA");
        };
        assert_eq!(g.fix, FixQuality::NoFix);
        assert_eq!(g.position, None);
        assert_eq!(g.time, None);
        assert_eq!(g.satellites, None);
    }

    #[test]
    fn unsupported_sentence_is_reported_not_rejected() {
        let s = with_checksum("GPGLL,4807.038,N,01131.000,E,123519,A,A");
        assert_eq!(parse_sentence(&s), Ok(Sentence::Unsupported));
    }

    #[test]
    fn rejects_bad_checksum() {
        let bad = GGA.replace("*47", "*48");
        assert_eq!(
            parse_sentence(&bad),
            Err(ParseError::ChecksumMismatch {
                expected: 0x47,
                found: 0x48
            })
        );
    }

    #[test]
    fn rejects_missing_start_and_checksum() {
        assert_eq!(parse_sentence("GPGGA,1*00"), Err(ParseError::MissingStart));
        assert_eq!(parse_sentence("$GPGGA,1"), Err(ParseError::MissingChecksum));
    }

    #[test]
    fn rejects_non_hex_checksum() {
        assert_eq!(
            parse_sentence("$GPGGA,1*ZZ"),
            Err(ParseError::InvalidChecksum)
        );
    }

    #[test]
    fn truncated_sentence_reports_missing_field() {
        let s = with_checksum("GPGGA,123519,4807.038,N");
        assert_eq!(
            parse_sentence(&s),
            Err(ParseError::MissingField("longitude"))
        );
    }

    #[test]
    fn rejects_out_of_range_minutes() {
        let s = with_checksum("GPGGA,123519,4870.038,N,01131.000,E,1,08,0.9,545.4,M,,,,");
        assert_eq!(
            parse_sentence(&s),
            Err(ParseError::InvalidField("latitude"))
        );
    }

    #[test]
    fn rejects_latitude_beyond_90() {
        // 91 degrees, 0 minutes.
        let s = with_checksum("GPGGA,123519,9100.000,N,01131.000,E,1,08,0.9,545.4,M,,,,");
        assert_eq!(
            parse_sentence(&s),
            Err(ParseError::InvalidField("position"))
        );
    }

    #[test]
    fn rejects_bad_hemisphere() {
        let s = with_checksum("GPGGA,123519,4807.038,X,01131.000,E,1,08,0.9,545.4,M,,,,");
        assert_eq!(
            parse_sentence(&s),
            Err(ParseError::InvalidField("latitude hemisphere"))
        );
    }

    #[test]
    fn rejects_unknown_fix_quality() {
        let s = with_checksum("GPGGA,123519,4807.038,N,01131.000,E,9,08,0.9,545.4,M,,,,");
        assert_eq!(
            parse_sentence(&s),
            Err(ParseError::InvalidField("fix quality"))
        );
    }

    #[test]
    fn fix_quality_has_position() {
        assert!(FixQuality::Gps.has_position());
        assert!(!FixQuality::NoFix.has_position());
        assert!(!FixQuality::Simulation.has_position());
    }
}
