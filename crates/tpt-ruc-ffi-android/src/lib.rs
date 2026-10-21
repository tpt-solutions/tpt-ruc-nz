// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Kotlin bindings for tpt-ruc-core, built on the `jni` crate.
//!
//! The Kotlin side declares these functions as `external` in the object
//! `com.tpt.ruc.RucNative` (see `samples/android/`). Money is returned as NZD
//! dollars in a `Double`, exact to the micro-dollar. Round it to cents for
//! display. A bad input throws a `RuntimeException` with the reason.

use jni::errors::{Error as JniError, ThrowRuntimeExAndDefault};
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jdouble, jint};
use jni::EnvUnowned;

/// A bad input, turned into a Java exception at the boundary.
#[derive(Debug)]
struct BadInput(String);

impl std::fmt::Display for BadInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BadInput {}

impl From<JniError> for BadInput {
    fn from(error: JniError) -> Self {
        Self(error.to_string())
    }
}

/// GST-inclusive charge for one trip, in NZD.
///
/// Kotlin signature: `external fun liabilityInclGstNzd(rateType: String, grossKg: Int,
/// electric: Boolean, year: Int, month: Int, day: Int, metres: Double): Double`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tpt_ruc_RucNative_liabilityInclGstNzd<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    rate_type: JString<'caller>,
    gross_kg: jint,
    electric: jboolean,
    year: jint,
    month: jint,
    day: jint,
    metres: jdouble,
) -> jdouble {
    unowned_env
        .with_env(|env| -> Result<f64, BadInput> {
            let rate_type = rate_type.try_to_string(env)?;
            let micros = tpt_ruc_ffi_common::liability_incl_gst_micros(
                &rate_type,
                to_u32(gross_kg, "grossKg")?,
                electric,
                to_u16(year, "year")?,
                to_u8(month, "month")?,
                to_u8(day, "day")?,
                metres,
            )
            .map_err(BadInput)?;
            Ok(micros as f64 / 1_000_000.0)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Distance in kilometres, rounded the way RUC bills it.
///
/// Kotlin signature: `external fun billableKm(metres: Double): Double`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_tpt_ruc_RucNative_billableKm<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    metres: jdouble,
) -> jdouble {
    unowned_env
        .with_env(|_env| -> Result<f64, BadInput> {
            tpt_ruc_ffi_common::billable_km(metres).map_err(BadInput)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

fn to_u32(value: jint, name: &str) -> Result<u32, BadInput> {
    u32::try_from(value).map_err(|_| BadInput(format!("{name} must not be negative")))
}

fn to_u16(value: jint, name: &str) -> Result<u16, BadInput> {
    u16::try_from(value).map_err(|_| BadInput(format!("{name} is out of range")))
}

fn to_u8(value: jint, name: &str) -> Result<u8, BadInput> {
    u8::try_from(value).map_err(|_| BadInput(format!("{name} is out of range")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_and_oversized_numbers_are_refused() {
        assert!(to_u32(-1, "grossKg").is_err());
        assert!(to_u8(1_300, "month").is_err());
        assert_eq!(to_u8(12, "month").unwrap(), 12);
        assert_eq!(to_u16(2026, "year").unwrap(), 2026);
    }
}
