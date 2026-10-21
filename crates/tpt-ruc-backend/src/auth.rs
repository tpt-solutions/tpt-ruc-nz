// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Who is calling, and what they may do.
//!
//! There are two kinds of caller. The owner holds the `TPT_RUC_API_KEY` and
//! has full access, which keeps single-owner self-hosting unchanged. Every other
//! caller is a user with a token issued when the user was created. Only a
//! SHA-256 hash of each token is stored. Tokens carry 256 random bits, so a
//! fast hash is enough here. Passwords would need a slow hash instead.

use http::header::AUTHORIZATION;
use http::HeaderMap;
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::AppState;

/// A user's role. The derived order runs from least to most access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Records trips and sees their own trips only.
    Driver,
    /// Sees every trip and runs reports.
    Manager,
    /// Sees every trip, runs reports and manages users.
    Admin,
}

impl Role {
    /// The name stored in the database and shown in the API.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Driver => "driver",
            Self::Manager => "manager",
            Self::Admin => "admin",
        }
    }

    /// Reads a role name. Returns `None` for anything else.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "driver" => Some(Self::Driver),
            "manager" => Some(Self::Manager),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }
}

/// The authenticated caller of one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    /// Signed in with the owner key. Has the admin role and no user record.
    Owner,
    /// Signed in with a user's token.
    User {
        /// The user's ID.
        id: String,
        /// The user's role.
        role: Role,
    },
}

impl Principal {
    /// The caller's role. The owner is an admin.
    pub fn role(&self) -> Role {
        match self {
            Self::Owner => Role::Admin,
            Self::User { role, .. } => *role,
        }
    }

    /// The caller's user ID, or `None` for the owner.
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::Owner => None,
            Self::User { id, .. } => Some(id),
        }
    }

    /// The user whose trips the caller may see, or `None` to see every trip.
    /// Drivers see only their own trips.
    pub fn trip_scope(&self) -> Option<&str> {
        if self.role() == Role::Driver {
            self.user_id()
        } else {
            None
        }
    }

    /// Fails with `Forbidden` unless the caller's role is at least `minimum`.
    pub fn require(&self, minimum: Role) -> Result<(), ApiError> {
        if self.role() >= minimum {
            Ok(())
        } else {
            Err(ApiError::Forbidden)
        }
    }
}

/// Works out who is calling from the `Authorization` header.
///
/// A missing or unknown token is `Unauthorized`, which the rate limiter counts
/// as a failed sign-in.
pub async fn resolve(state: &AppState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    let token = bearer(headers).ok_or(ApiError::Unauthorized)?;
    if constant_time_eq(token.as_bytes(), state.api_key.as_bytes()) {
        return Ok(Principal::Owner);
    }

    let hash = token_hash(token);
    let found = state
        .db
        .run(move |connection| {
            Ok(connection
                .query_row(
                    "SELECT id, role FROM users WHERE token_hash = ?1",
                    params![hash],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?)
        })
        .await?;

    match found {
        Some((id, role)) => {
            let role = Role::parse(&role).ok_or(ApiError::Internal)?;
            Ok(Principal::User { id, role })
        }
        None => Err(ApiError::Unauthorized),
    }
}

/// The token from `Authorization: Bearer <token>`, if the header has that form.
fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

/// The hex SHA-256 digest of a token. This is what the database stores.
pub fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// `bytes` random bytes from the operating system, as lowercase hex.
pub fn random_hex(bytes: usize) -> Result<String, ApiError> {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).map_err(|error| {
        tracing::error!(%error, "the operating system random source failed");
        ApiError::Internal
    })?;
    Ok(hex(&buffer))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Compares two byte strings without stopping at the first difference.
///
/// The length check returns early, so the key's length can leak. That is
/// acceptable for a random key: the length is not the secret.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_bytes_match() {
        assert!(constant_time_eq(b"secret-key", b"secret-key"));
    }

    #[test]
    fn different_bytes_do_not_match() {
        assert!(!constant_time_eq(b"secret-key", b"secret-kez"));
        assert!(!constant_time_eq(b"secret-key", b"secret"));
        assert!(!constant_time_eq(b"", b"secret"));
    }

    #[test]
    fn roles_are_ordered_by_access() {
        assert!(Role::Driver < Role::Manager);
        assert!(Role::Manager < Role::Admin);
        assert_eq!(Role::parse("manager"), Some(Role::Manager));
        assert_eq!(Role::parse("owner"), None);
        assert_eq!(Role::Driver.as_str(), "driver");
    }

    #[test]
    fn drivers_are_scoped_to_their_own_trips() {
        let driver = Principal::User {
            id: "u1".to_owned(),
            role: Role::Driver,
        };
        let manager = Principal::User {
            id: "u2".to_owned(),
            role: Role::Manager,
        };
        assert_eq!(driver.trip_scope(), Some("u1"));
        assert_eq!(manager.trip_scope(), None);
        assert_eq!(Principal::Owner.trip_scope(), None);
    }

    #[test]
    fn access_checks_use_the_role() {
        let driver = Principal::User {
            id: "u1".to_owned(),
            role: Role::Driver,
        };
        assert!(driver.require(Role::Driver).is_ok());
        assert!(matches!(
            driver.require(Role::Manager),
            Err(ApiError::Forbidden)
        ));
        assert!(Principal::Owner.require(Role::Admin).is_ok());
    }

    #[test]
    fn token_hashes_are_stable_and_do_not_reveal_the_token() {
        let hash = token_hash("tpt_example");
        assert_eq!(hash, token_hash("tpt_example"));
        assert_ne!(hash, "tpt_example");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn random_tokens_have_the_requested_length_and_differ() {
        let a = random_hex(32).unwrap();
        let b = random_hex(32).unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}
