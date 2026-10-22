// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The device seed, kept in flash so the device identity survives a reboot.
//!
//! The seed takes sector 0 of the storage it is given. The record in that sector is:
//!
//! `[magic: u8][seed: 32 bytes][crc32 of seed: u32 LE]`
//!
//! The magic byte is written last. Until it is written, the seed has not been
//! returned to anyone, so a power cut during the first boot leaves a blank
//! magic and the next boot simply makes a new seed. A magic byte that is not
//! blank, with a bad checksum or an unknown value, means the record is damaged.
//! The device then refuses to make a new seed, because that would silently
//! change its identity.
//!
//! The random source is the chip's hardware RNG, behind [`RandomSource`].

use crate::flash::SectorStorage;
use crate::identity::{SeedSource, SEED_LEN};
use crate::log::crc32;

/// First byte of a valid seed record.
pub const SEED_MAGIC: u8 = 0x5d;

/// Length of the whole seed record.
const RECORD_LEN: usize = 1 + SEED_LEN + 4;

/// Where the seed record starts within sector 0.
const BODY: usize = 1;

/// Bytes in flash that are blank after an erase.
const BLANK: u8 = 0xff;

/// Source of random bytes for the seed. The chip driver implements this.
pub trait RandomSource {
    /// Fills `buf` with random bytes.
    fn fill(&mut self, buf: &mut [u8]);
}

/// Why the seed could not be loaded or made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedError {
    /// The flash could not be read or written.
    Storage,
    /// The seed record is present but damaged. It is not replaced.
    Damaged,
}

/// Loads the device seed from sector 0 of `storage`, or makes and stores one on first use.
pub struct FlashSeed<'a, S, R> {
    storage: &'a mut S,
    rng: R,
}

impl<'a, S: SectorStorage, R: RandomSource> FlashSeed<'a, S, R> {
    /// Uses sector 0 of `storage` for the seed.
    pub fn new(storage: &'a mut S, rng: R) -> Self {
        Self { storage, rng }
    }

    /// Makes a seed and stores it. Sector 0 is erased first, and the magic byte is written last.
    fn create(&mut self) -> Result<[u8; SEED_LEN], SeedError> {
        if !self.storage.erase(0) {
            return Err(SeedError::Storage);
        }
        let mut seed = [0u8; SEED_LEN];
        self.rng.fill(&mut seed);

        let mut body = [0u8; RECORD_LEN - BODY];
        body[..SEED_LEN].copy_from_slice(&seed);
        body[SEED_LEN..].copy_from_slice(&crc32(&seed).to_le_bytes());
        if !self.storage.write(0, BODY, &body) {
            return Err(SeedError::Storage);
        }
        if !self.storage.write(0, 0, &[SEED_MAGIC]) {
            return Err(SeedError::Storage);
        }

        // Read the record back, so a seed that did not reach flash is never used.
        let mut record = [0u8; RECORD_LEN];
        if !self.storage.read(0, 0, &mut record) || !valid(&record) {
            return Err(SeedError::Storage);
        }
        Ok(seed)
    }
}

impl<S: SectorStorage, R: RandomSource> SeedSource for FlashSeed<'_, S, R> {
    type Error = SeedError;

    fn seed(&mut self) -> Result<[u8; SEED_LEN], SeedError> {
        let mut record = [0u8; RECORD_LEN];
        if !self.storage.read(0, 0, &mut record) {
            return Err(SeedError::Storage);
        }
        match record[0] {
            SEED_MAGIC if valid(&record) => {
                let mut seed = [0u8; SEED_LEN];
                seed.copy_from_slice(&record[BODY..BODY + SEED_LEN]);
                Ok(seed)
            }
            SEED_MAGIC => Err(SeedError::Damaged),
            BLANK => self.create(),
            _ => Err(SeedError::Damaged),
        }
    }
}

/// True when the record has the magic byte and a checksum that matches its seed.
fn valid(record: &[u8; RECORD_LEN]) -> bool {
    if record[0] != SEED_MAGIC {
        return false;
    }
    let seed = &record[BODY..BODY + SEED_LEN];
    let stored = u32::from_le_bytes([
        record[BODY + SEED_LEN],
        record[BODY + SEED_LEN + 1],
        record[BODY + SEED_LEN + 2],
        record[BODY + SEED_LEN + 3],
    ]);
    stored == crc32(seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::tests::RamSectors;
    use crate::identity::load_identity;

    /// A counter-based random source, so tests can tell seeds apart.
    struct Counting(u8);

    impl RandomSource for Counting {
        fn fill(&mut self, buf: &mut [u8]) {
            for byte in buf.iter_mut() {
                self.0 = self.0.wrapping_add(1);
                *byte = self.0;
            }
        }
    }

    fn storage() -> RamSectors {
        RamSectors::new(2, 128)
    }

    #[test]
    fn the_first_call_makes_a_seed_and_later_calls_return_it() {
        let mut storage = storage();
        let first = FlashSeed::new(&mut storage, Counting(0)).seed().unwrap();
        // A different random source must not change the stored seed.
        let second = FlashSeed::new(&mut storage, Counting(100)).seed().unwrap();
        assert_eq!(first, second);
        assert_ne!(first, [0u8; SEED_LEN]);
    }

    #[test]
    fn the_identity_is_the_same_on_every_boot() {
        let mut storage = storage();
        let mut source = FlashSeed::new(&mut storage, Counting(0));
        let first = load_identity(&mut source).unwrap().public_key();
        let mut source = FlashSeed::new(&mut storage, Counting(50));
        let second = load_identity(&mut source).unwrap().public_key();
        assert_eq!(first, second);
    }

    #[test]
    fn an_interrupted_first_boot_makes_a_new_seed() {
        let mut storage = storage();
        // Power failed after the body was written and before the magic byte.
        assert!(storage.write(0, BODY, &[0x42; RECORD_LEN - BODY]));
        let seed = FlashSeed::new(&mut storage, Counting(0)).seed().unwrap();
        assert_eq!(
            FlashSeed::new(&mut storage, Counting(9)).seed().unwrap(),
            seed
        );
    }

    #[test]
    fn a_seed_with_a_bad_checksum_is_damaged_not_replaced() {
        let mut storage = storage();
        FlashSeed::new(&mut storage, Counting(0)).seed().unwrap();
        // Bit rot in the stored seed. The checksum no longer matches, so the seed is not replaced.
        storage.bytes[BODY] ^= 0x01;
        assert_eq!(
            FlashSeed::new(&mut storage, Counting(0)).seed(),
            Err(SeedError::Damaged)
        );
    }

    #[test]
    fn an_unknown_magic_byte_is_damaged() {
        let mut storage = storage();
        assert!(storage.write(0, 0, &[0x11]));
        assert_eq!(
            FlashSeed::new(&mut storage, Counting(0)).seed(),
            Err(SeedError::Damaged)
        );
    }

    #[test]
    fn a_flash_failure_is_reported() {
        let mut storage = storage();
        storage.ops_left = 0;
        assert_eq!(
            FlashSeed::new(&mut storage, Counting(0)).seed(),
            Err(SeedError::Storage)
        );
    }
}
