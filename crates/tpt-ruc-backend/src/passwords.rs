// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Password hashing for browser sign-in.
//!
//! Passwords are hashed with Argon2id, the default parameters of the `argon2`
//! crate, and a fresh random salt for each one. Tokens do not use this module:
//! they carry 256 random bits and are stored as a plain SHA-256 hash (see
//! [`crate::auth`]).

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

use crate::error::ApiError;

/// Shortest password accepted, in bytes.
pub const MIN_PASSWORD_LEN: usize = 12;
/// Longest password accepted, in bytes. Argon2 cost grows with length, so this caps the work per request.
pub const MAX_PASSWORD_LEN: usize = 256;

/// Salt length in bytes. Argon2 recommends at least 16.
const SALT_LEN: usize = 16;

/// Checks a password against the length rules. Returns a message for the caller when it is refused.
pub fn check_policy(password: &str) -> Result<(), ApiError> {
    if password.len() < MIN_PASSWORD_LEN || password.len() > MAX_PASSWORD_LEN {
        return Err(ApiError::BadRequest(format!(
            "password must be {MIN_PASSWORD_LEN} to {MAX_PASSWORD_LEN} bytes"
        )));
    }
    Ok(())
}

/// Hashes a password for storage. The result holds the algorithm, parameters and salt.
pub fn hash(password: &str) -> Result<String, ApiError> {
    let mut salt_bytes = [0u8; SALT_LEN];
    getrandom::fill(&mut salt_bytes).map_err(|error| {
        tracing::error!(%error, "the operating system random source failed");
        ApiError::Internal
    })?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|error| {
        tracing::error!(%error, "could not encode a password salt");
        ApiError::Internal
    })?;
    Argon2::default()
        .hash_password(password.as_bytes(), salt.as_salt())
        .map(|hash| hash.to_string())
        .map_err(|error| {
            tracing::error!(%error, "could not hash a password");
            ApiError::Internal
        })
}

/// Checks a password against a stored hash. A malformed stored hash never matches.
pub fn verify(password: &str, stored: &str) -> bool {
    match PasswordHash::new(stored) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hash_verifies_only_its_own_password() {
        let stored = hash("correct horse battery").unwrap();
        assert!(verify("correct horse battery", &stored));
        assert!(!verify("correct horse batterz", &stored));
        assert!(!verify("", &stored));
    }

    #[test]
    fn the_same_password_gets_a_different_salt_each_time() {
        let a = hash("correct horse battery").unwrap();
        let b = hash("correct horse battery").unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("$argon2id$"));
    }

    #[test]
    fn a_malformed_stored_hash_never_matches() {
        assert!(!verify("anything at all", "not-a-hash"));
        assert!(!verify("anything at all", ""));
    }

    #[test]
    fn password_length_is_checked_in_bytes() {
        assert!(check_policy(&"a".repeat(MIN_PASSWORD_LEN)).is_ok());
        assert!(check_policy(&"a".repeat(MIN_PASSWORD_LEN - 1)).is_err());
        assert!(check_policy(&"a".repeat(MAX_PASSWORD_LEN)).is_ok());
        assert!(check_policy(&"a".repeat(MAX_PASSWORD_LEN + 1)).is_err());
    }
}
