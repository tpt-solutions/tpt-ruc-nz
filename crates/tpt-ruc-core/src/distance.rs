// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Great-circle distance between two coordinates.

use crate::coord::Coordinate;

/// Mean Earth radius in metres (IUGG mean radius).
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Great-circle distance between two points in metres, using the haversine formula.
///
/// Accuracy is within about 0.5% because the Earth is modelled as a sphere.
/// That is adequate for trip accumulation but not for legal distance records.
pub fn haversine_m(a: Coordinate, b: Coordinate) -> f64 {
    let lat1 = libm::cos(to_radians(a.lat.degrees()));
    let lat2 = libm::cos(to_radians(b.lat.degrees()));
    let dlat = to_radians(b.lat.degrees() - a.lat.degrees());
    let dlon = to_radians(b.lon.degrees() - a.lon.degrees());

    let sin_dlat = libm::sin(dlat / 2.0);
    let sin_dlon = libm::sin(dlon / 2.0);
    let h = sin_dlat * sin_dlat + lat1 * lat2 * sin_dlon * sin_dlon;
    // Clamp to guard against floating-point drift just outside 0..=1.
    let h = h.clamp(0.0, 1.0);

    2.0 * EARTH_RADIUS_M * libm::asin(libm::sqrt(h))
}

/// Degrees to radians. Written out by hand because `f64::to_radians` is not in `core`.
fn to_radians(degrees: f64) -> f64 {
    degrees * core::f64::consts::PI / 180.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(lat: f64, lon: f64) -> Coordinate {
        Coordinate::new(lat, lon).unwrap()
    }

    #[test]
    fn zero_distance_for_same_point() {
        let p = c(-41.2865, 174.7762);
        assert_eq!(haversine_m(p, p), 0.0);
    }

    #[test]
    fn one_degree_of_latitude_is_about_111_km() {
        let d = haversine_m(c(0.0, 0.0), c(1.0, 0.0));
        // 1 degree of arc on the mean-radius sphere.
        let expected = EARTH_RADIUS_M.to_radians();
        assert!((d - expected).abs() < 1e-6, "got {d}, expected {expected}");
    }

    #[test]
    fn wellington_to_auckland_is_about_490_km() {
        // Wellington (-41.2865, 174.7762) to Auckland (-36.8485, 174.7633).
        let d = haversine_m(c(-41.2865, 174.7762), c(-36.8485, 174.7633)) / 1000.0;
        assert!((d - 490.0).abs() < 5.0, "got {d} km");
    }

    #[test]
    fn symmetric() {
        let a = c(-45.0, 170.0);
        let b = c(-36.0, 175.0);
        assert!((haversine_m(a, b) - haversine_m(b, a)).abs() < 1e-9);
    }

    #[test]
    fn antipodal_points_do_not_nan() {
        let d = haversine_m(c(0.0, 0.0), c(0.0, 180.0));
        assert!(d.is_finite());
        assert!((d - core::f64::consts::PI * EARTH_RADIUS_M).abs() < 1e-3);
    }
}
