// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Validated WGS-84 coordinates.

use core::fmt;

/// Error returned when a latitude or longitude is out of range or not finite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateError {
    /// Latitude outside `-90.0..=90.0`, or NaN.
    Latitude,
    /// Longitude outside `-180.0..=180.0`, or NaN.
    Longitude,
}

impl fmt::Display for CoordinateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Latitude => f.write_str("latitude must be within -90..=90 degrees"),
            Self::Longitude => f.write_str("longitude must be within -180..=180 degrees"),
        }
    }
}

/// A latitude in decimal degrees, guaranteed to be within `-90..=90`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "f64", into = "f64"))]
pub struct Latitude(f64);

impl TryFrom<f64> for Latitude {
    type Error = CoordinateError;

    fn try_from(degrees: f64) -> Result<Self, Self::Error> {
        Self::new(degrees)
    }
}

impl From<Latitude> for f64 {
    fn from(lat: Latitude) -> f64 {
        lat.0
    }
}

impl Latitude {
    /// Creates a latitude, rejecting values outside `-90..=90` and NaN.
    pub fn new(degrees: f64) -> Result<Self, CoordinateError> {
        if (-90.0..=90.0).contains(&degrees) {
            Ok(Self(degrees))
        } else {
            Err(CoordinateError::Latitude)
        }
    }

    /// Returns the value in decimal degrees.
    pub fn degrees(self) -> f64 {
        self.0
    }
}

/// A longitude in decimal degrees, guaranteed to be within `-180..=180`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "f64", into = "f64"))]
pub struct Longitude(f64);

impl TryFrom<f64> for Longitude {
    type Error = CoordinateError;

    fn try_from(degrees: f64) -> Result<Self, Self::Error> {
        Self::new(degrees)
    }
}

impl From<Longitude> for f64 {
    fn from(lon: Longitude) -> f64 {
        lon.0
    }
}

impl Longitude {
    /// Creates a longitude, rejecting values outside `-180..=180` and NaN.
    pub fn new(degrees: f64) -> Result<Self, CoordinateError> {
        if (-180.0..=180.0).contains(&degrees) {
            Ok(Self(degrees))
        } else {
            Err(CoordinateError::Longitude)
        }
    }

    /// Returns the value in decimal degrees.
    pub fn degrees(self) -> f64 {
        self.0
    }
}

/// A validated latitude and longitude pair.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Coordinate {
    /// Latitude, positive north.
    pub lat: Latitude,
    /// Longitude, positive east.
    pub lon: Longitude,
}

impl Coordinate {
    /// Creates a coordinate from decimal degrees, validating both components.
    pub fn new(lat_deg: f64, lon_deg: f64) -> Result<Self, CoordinateError> {
        Ok(Self {
            lat: Latitude::new(lat_deg)?,
            lon: Longitude::new(lon_deg)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bounds() {
        assert!(Coordinate::new(90.0, 180.0).is_ok());
        assert!(Coordinate::new(-90.0, -180.0).is_ok());
    }

    #[test]
    fn rejects_out_of_range() {
        assert_eq!(Latitude::new(90.1), Err(CoordinateError::Latitude));
        assert_eq!(Longitude::new(-180.5), Err(CoordinateError::Longitude));
    }

    #[test]
    fn rejects_nan() {
        assert_eq!(Latitude::new(f64::NAN), Err(CoordinateError::Latitude));
        assert_eq!(Longitude::new(f64::NAN), Err(CoordinateError::Longitude));
    }
}
