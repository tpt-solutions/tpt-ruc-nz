// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Per-device Ed25519 identity.
//!
//! Each tracker has its own key pair. It signs the data it hands to the phone,
//! so the phone and, later, the backend can check that a segment came from this
//! device and was not changed on the way. The key pair is derived from a 32-byte
//! seed. The seed must come from the hardware random number generator on first
//! boot and be kept where only the firmware can read it. This module does not
//! generate or store the seed. That needs the chip's driver.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// Length of a public key, in bytes.
pub const PUBLIC_KEY_LEN: usize = 32;
/// Length of a signature, in bytes.
pub const SIGNATURE_LEN: usize = 64;
/// Length of the seed the key pair is derived from, in bytes.
pub const SEED_LEN: usize = 32;

/// The device's signing key. The secret is never printed or exported.
pub struct DeviceIdentity {
    key: SigningKey,
}

impl core::fmt::Debug for DeviceIdentity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DeviceIdentity")
            .field("public_key", &self.public_key())
            .finish_non_exhaustive()
    }
}

impl DeviceIdentity {
    /// Derives the key pair from a 32-byte seed.
    pub fn from_seed(seed: &[u8; SEED_LEN]) -> Self {
        Self {
            key: SigningKey::from_bytes(seed),
        }
    }

    /// The public key, which the phone stores to check this device's signatures.
    pub fn public_key(&self) -> [u8; PUBLIC_KEY_LEN] {
        self.key.verifying_key().to_bytes()
    }

    /// Signs `message`. The signature is deterministic for a given key and message.
    pub fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_LEN] {
        self.key.sign(message).to_bytes()
    }
}

/// Checks `signature` over `message` against `public_key`. Malformed keys fail the check.
pub fn verify(
    public_key: &[u8; PUBLIC_KEY_LEN],
    message: &[u8],
    signature: &[u8; SIGNATURE_LEN],
) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(public_key) else {
        return false;
    };
    key.verify(message, &Signature::from_bytes(signature))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 8032, test 1: the empty message. Values checked against an independent implementation.
    const SEED_RFC8032: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];

    fn hex<const N: usize>(text: &str) -> [u8; N] {
        let mut out = [0u8; N];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn matches_the_rfc_8032_test_vector() {
        let identity = DeviceIdentity::from_seed(&SEED_RFC8032);
        assert_eq!(
            identity.public_key(),
            hex::<32>("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
        );
        assert_eq!(
            identity.sign(b""),
            hex::<64>(
                "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
            )
        );
    }

    #[test]
    fn a_signature_verifies_and_a_changed_message_does_not() {
        let identity = DeviceIdentity::from_seed(&SEED_RFC8032);
        let public_key = identity.public_key();
        let signature = identity.sign(b"segment 1");
        assert!(verify(&public_key, b"segment 1", &signature));
        assert!(!verify(&public_key, b"segment 2", &signature));
    }

    #[test]
    fn a_signature_from_another_device_is_refused() {
        let ours = DeviceIdentity::from_seed(&SEED_RFC8032);
        let theirs = DeviceIdentity::from_seed(&[7u8; 32]);
        let signature = theirs.sign(b"segment 1");
        assert!(!verify(&ours.public_key(), b"segment 1", &signature));
    }

    #[test]
    fn a_malformed_public_key_fails_the_check() {
        // A y-coordinate of 2 has no valid point on the curve, so it is not a key.
        let mut bogus = [0u8; 32];
        bogus[0] = 2;
        assert!(!verify(&bogus, b"x", &[0u8; 64]));
    }

    #[test]
    fn debug_output_hides_the_secret() {
        let identity = DeviceIdentity::from_seed(&SEED_RFC8032);
        let text = std::format!("{identity:?}");
        assert!(text.contains("public_key"));
        assert!(!text.contains("9d61b19d"));
    }
}
