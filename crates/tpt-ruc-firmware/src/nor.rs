// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Byte-addressed sectors over a NOR flash that reads and writes whole words.
//!
//! The chip driver behind `embedded-storage`'s [`NorFlash`] (esp-storage on the
//! ESP32 parts) only reads and writes aligned 4-byte words. This module gives the
//! flash log the byte-addressed [`SectorStorage`] it expects. A write that does
//! not start or end on a word boundary reads the whole word, clears only the bits
//! the new bytes need, and writes the word back. NOR flash can clear bits without
//! an erase, so the bytes already in that word stay as they were.
//!
//! The window starts at a byte address that is a multiple of the erase size.
//! [`NorSectors::after`] drops leading sectors, so one partition can hold a
//! sector for the device seed and then the flash log.

use core::cell::RefCell;

use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};

use crate::flash::SectorStorage;

/// Bytes per flash word. Reads and writes are aligned to this on the chips we target.
const WORD: usize = 4;

/// Erase sectors of a NOR flash, as [`SectorStorage`].
pub struct NorSectors<F> {
    flash: RefCell<F>,
    /// Byte address of sector 0 of this window.
    base: u32,
    sectors: usize,
    sector_size: usize,
}

impl<F: NorFlash> NorSectors<F> {
    /// Covers `sectors` erase sectors of `flash`, starting at byte address `base`.
    ///
    /// Returns `None` when `base` is not on an erase boundary, or when the driver
    /// does not write in 4-byte words.
    pub fn new(flash: F, base: u32, sectors: usize) -> Option<Self> {
        let sector_size = F::ERASE_SIZE;
        if F::WRITE_SIZE != WORD || sector_size == 0 || !(base as usize).is_multiple_of(sector_size)
        {
            return None;
        }
        Some(Self {
            flash: RefCell::new(flash),
            base,
            sectors,
            sector_size,
        })
    }

    /// The window without its first `skip` sectors. `None` when fewer than `skip` sectors exist.
    pub fn after(self, skip: usize) -> Option<Self> {
        let sectors = self.sectors.checked_sub(skip)?;
        Some(Self {
            base: self.base + (skip * self.sector_size) as u32,
            sectors,
            ..self
        })
    }

    /// Byte address in the flash of `offset` within `sector`.
    fn address(&self, sector: usize, offset: usize) -> u32 {
        self.base + (sector * self.sector_size + offset) as u32
    }

    /// True when `offset..offset + len` lies inside `sector`.
    fn in_bounds(&self, sector: usize, offset: usize, len: usize) -> bool {
        sector < self.sectors && offset + len <= self.sector_size
    }
}

impl<F: NorFlash> SectorStorage for NorSectors<F> {
    fn sector_count(&self) -> usize {
        self.sectors
    }

    fn sector_size(&self) -> usize {
        self.sector_size
    }

    fn read(&self, sector: usize, offset: usize, buf: &mut [u8]) -> bool {
        if !self.in_bounds(sector, offset, buf.len()) {
            return false;
        }
        let address = self.address(sector, offset);
        read_bytes(&mut *self.flash.borrow_mut(), address, buf)
    }

    fn write(&mut self, sector: usize, offset: usize, data: &[u8]) -> bool {
        if !self.in_bounds(sector, offset, data.len()) {
            return false;
        }
        let address = self.address(sector, offset);
        write_bytes(self.flash.get_mut(), address, data)
    }

    fn erase(&mut self, sector: usize) -> bool {
        if sector >= self.sectors {
            return false;
        }
        let from = self.address(sector, 0);
        self.flash
            .get_mut()
            .erase(from, from + self.sector_size as u32)
            .is_ok()
    }
}

/// Reads `buf` from `address`, one aligned word at a time.
fn read_bytes<F: ReadNorFlash>(flash: &mut F, address: u32, buf: &mut [u8]) -> bool {
    let mut done = 0;
    while done < buf.len() {
        let at = address + done as u32;
        let word = at & !(WORD as u32 - 1);
        let skip = (at - word) as usize;
        let take = (WORD - skip).min(buf.len() - done);
        let mut bytes = [0u8; WORD];
        if flash.read(word, &mut bytes).is_err() {
            return false;
        }
        buf[done..done + take].copy_from_slice(&bytes[skip..skip + take]);
        done += take;
    }
    true
}

/// Writes `data` at `address`. Each touched word is read, merged and written back.
fn write_bytes<F: NorFlash>(flash: &mut F, address: u32, data: &[u8]) -> bool {
    let mut done = 0;
    while done < data.len() {
        let at = address + done as u32;
        let word = at & !(WORD as u32 - 1);
        let skip = (at - word) as usize;
        let take = (WORD - skip).min(data.len() - done);
        let mut bytes = [0u8; WORD];
        if flash.read(word, &mut bytes).is_err() {
            return false;
        }
        let merged = merge_word(bytes, skip, &data[done..done + take]);
        // Skip the write when nothing changes, which saves a program cycle.
        if merged != bytes && flash.write(word, &merged).is_err() {
            return false;
        }
        done += take;
    }
    true
}

/// Clears, in `word` starting at `skip`, every bit that is clear in `data`.
/// Bytes outside `skip..skip + data.len()` are unchanged.
fn merge_word(mut word: [u8; WORD], skip: usize, data: &[u8]) -> [u8; WORD] {
    for (byte, new) in word[skip..].iter_mut().zip(data) {
        *byte &= *new;
    }
    word
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_storage::nor_flash::{ErrorType, NorFlashError, NorFlashErrorKind};

    /// Erase size of the RAM flash. Small, so a few records span several sectors.
    const RAM_SECTOR: usize = 128;

    #[derive(Debug)]
    struct RamError;

    impl NorFlashError for RamError {
        fn kind(&self) -> NorFlashErrorKind {
            NorFlashErrorKind::Other
        }
    }

    /// RAM that behaves like the chip driver: word writes, erase to `0xFF`, and
    /// writes that can only clear bits.
    struct RamNor {
        bytes: std::vec::Vec<u8>,
        writes: usize,
    }

    impl RamNor {
        fn new(sectors: usize) -> Self {
            Self {
                bytes: std::vec![0xff; sectors * RAM_SECTOR],
                writes: 0,
            }
        }
    }

    impl ErrorType for RamNor {
        type Error = RamError;
    }

    impl ReadNorFlash for RamNor {
        const READ_SIZE: usize = 1;

        fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), RamError> {
            let start = offset as usize;
            let source = self.bytes.get(start..start + bytes.len()).ok_or(RamError)?;
            bytes.copy_from_slice(source);
            Ok(())
        }

        fn capacity(&self) -> usize {
            self.bytes.len()
        }
    }

    impl NorFlash for RamNor {
        const WRITE_SIZE: usize = WORD;
        const ERASE_SIZE: usize = RAM_SECTOR;

        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), RamError> {
            let start = offset as usize;
            if !start.is_multiple_of(WORD) || !bytes.len().is_multiple_of(WORD) {
                return Err(RamError);
            }
            let target = self
                .bytes
                .get_mut(start..start + bytes.len())
                .ok_or(RamError)?;
            for (cell, new) in target.iter_mut().zip(bytes) {
                *cell &= *new;
            }
            self.writes += 1;
            Ok(())
        }

        fn erase(&mut self, from: u32, to: u32) -> Result<(), RamError> {
            let (from, to) = (from as usize, to as usize);
            if from % RAM_SECTOR != 0 || to % RAM_SECTOR != 0 {
                return Err(RamError);
            }
            self.bytes.get_mut(from..to).ok_or(RamError)?.fill(0xff);
            Ok(())
        }
    }

    fn window(sectors: usize) -> NorSectors<RamNor> {
        NorSectors::new(RamNor::new(sectors), 0, sectors).unwrap()
    }

    #[test]
    fn a_base_off_the_erase_boundary_is_refused() {
        assert!(NorSectors::new(RamNor::new(4), 4, 2).is_none());
        assert!(NorSectors::new(RamNor::new(4), RAM_SECTOR as u32, 2).is_some());
    }

    #[test]
    fn unaligned_bytes_round_trip_through_word_writes() {
        let mut storage = window(2);
        assert!(storage.write(0, 1, &[1, 2, 3, 4, 5]));
        assert!(storage.write(0, 7, &[9]));
        let mut out = [0u8; 8];
        assert!(storage.read(0, 0, &mut out));
        assert_eq!(out, [0xff, 1, 2, 3, 4, 5, 0xff, 9]);
    }

    #[test]
    fn a_second_write_clears_bits_without_erasing_the_rest_of_the_word() {
        let mut storage = window(1);
        assert!(storage.write(0, 0, &[0xf0]));
        assert!(storage.write(0, 1, &[0x0f]));
        let mut out = [0u8; 2];
        assert!(storage.read(0, 0, &mut out));
        assert_eq!(out, [0xf0, 0x0f]);
    }

    #[test]
    fn writes_and_reads_outside_the_window_are_refused() {
        let mut storage = window(2);
        assert!(!storage.write(2, 0, &[1]));
        assert!(!storage.write(0, RAM_SECTOR, &[1]));
        assert!(!storage.write(0, RAM_SECTOR - 1, &[1, 2]));
        let mut out = [0u8; 2];
        assert!(!storage.read(1, RAM_SECTOR - 1, &mut out));
    }

    #[test]
    fn erase_clears_one_sector_only() {
        let mut storage = window(2);
        assert!(storage.write(0, 0, &[1, 2]));
        assert!(storage.write(1, 0, &[3, 4]));
        assert!(storage.erase(0));
        let mut out = [0u8; 2];
        assert!(storage.read(0, 0, &mut out));
        assert_eq!(out, [0xff, 0xff]);
        assert!(storage.read(1, 0, &mut out));
        assert_eq!(out, [3, 4]);
    }

    #[test]
    fn after_drops_leading_sectors() {
        let mut storage = window(3);
        assert!(storage.write(1, 0, &[7]));
        let rest = storage.after(1).unwrap();
        assert_eq!(rest.sector_count(), 2);
        let mut out = [0u8; 1];
        assert!(rest.read(0, 0, &mut out));
        assert_eq!(out, [7]);
        assert!(rest.after(3).is_none());
    }

    #[test]
    fn an_unchanged_word_is_not_written_again() {
        let mut storage = window(1);
        assert!(storage.write(0, 0, &[0x55; 4]));
        let writes = storage.flash.get_mut().writes;
        assert!(storage.write(0, 0, &[0xff; 4]));
        assert_eq!(storage.flash.get_mut().writes, writes);
    }

    #[test]
    fn the_flash_log_works_over_word_writes() {
        use crate::flash::SectorLog;
        use tpt_ruc_core::Segment;

        let segment = |i: u64| Segment {
            start_ms: i * 1_000,
            end_ms: i * 1_000 + 900,
            distance_m: i as f64,
        };
        let mut log = SectorLog::open(window(4));
        for i in 1..=12 {
            log.append(&segment(i)).unwrap();
        }
        let reopened = SectorLog::open(log.into_storage());
        let mut replayed = std::vec::Vec::new();
        reopened.replay(|s| replayed.push(s));
        let expected: std::vec::Vec<Segment> = (1..=12).map(segment).collect();
        assert_eq!(replayed, expected);
    }
}
