// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Trip segment storage on NOR flash, with wear levelling.
//!
//! The storage is split into sectors. Each sector starts with a header that
//! has a sequence number, and then holds whole segment records (the format in
//! [`crate::log`]). A record never spans two sectors.
//!
//! * **Wear levelling.** Sectors are used in round-robin order. Each erase
//!   therefore moves on to the next sector, so the erase count stays within one
//!   of every other sector's.
//! * **Power loss.** A sector header is written after its erase. A header cut
//!   short by power loss fails its CRC, and the sector counts as free. A record
//!   cut short leaves a dirty tail. Flash cannot set a bit back to 1 without an
//!   erase, so a sector with a dirty tail is sealed and never written again
//!   until it is retired.
//! * **Retirement.** Only the oldest sector can be erased, and the caller should
//!   do so only after the phone has synced its segments. Until then, the
//!   storage keeps the data.
//!
//! The hardware driver is behind [`SectorStorage`]. This module is tested on the
//! host with RAM-backed storage that can cut power at any write or erase.

use crate::log::{self, decode_segment, encode_segment, LogError};
use tpt_ruc_core::Segment;

/// First byte of every sector header.
pub const SECTOR_MAGIC: u8 = 0x5e;

/// Bytes at the start of each sector: magic, sequence number and CRC.
pub const SECTOR_HEADER_LEN: usize = 1 + 4 + 4;

/// Sector-addressed storage. Flash drivers implement this.
pub trait SectorStorage {
    /// Number of sectors.
    fn sector_count(&self) -> usize;
    /// Bytes in each sector.
    fn sector_size(&self) -> usize;
    /// Fills `buf` from `offset` within `sector`. Returns `false` if out of bounds.
    fn read(&self, sector: usize, offset: usize, buf: &mut [u8]) -> bool;
    /// Writes `data` at `offset` within `sector`. Writes may only clear bits,
    /// so the target must be erased. Returns `false` on failure.
    fn write(&mut self, sector: usize, offset: usize, data: &[u8]) -> bool;
    /// Sets every byte of `sector` to `0xFF`. Returns `false` on failure.
    fn erase(&mut self, sector: usize) -> bool;
}

/// Trip segments in sectors of flash, oldest first.
pub struct SectorLog<S> {
    storage: S,
    /// Sector that new records go to, if it can take more.
    current: Option<usize>,
    /// Bytes used in `current`, including its header.
    used: usize,
    /// The sector most recently opened. Round-robin starts after it.
    last_opened: Option<usize>,
    /// Sequence number for the next sector opened.
    next_seq: u32,
}

impl<S: SectorStorage> SectorLog<S> {
    /// Opens the log. Scans every sector to find the newest one and where its records end.
    pub fn open(storage: S) -> Self {
        let mut newest: Option<(u32, usize)> = None;
        for sector in 0..storage.sector_count() {
            if let Some(seq) = read_header(&storage, sector) {
                if newest.is_none_or(|(best, _)| seq > best) {
                    newest = Some((seq, sector));
                }
            }
        }

        let mut log = Self {
            storage,
            current: None,
            used: 0,
            last_opened: None,
            next_seq: 1,
        };
        if let Some((seq, sector)) = newest {
            let (end, clean) = scan_end(&log.storage, sector);
            log.used = end;
            log.current = clean.then_some(sector);
            log.last_opened = Some(sector);
            log.next_seq = seq.wrapping_add(1);
        }
        log
    }

    /// Appends one segment. Opens a new sector when the current one is full or sealed.
    pub fn append(&mut self, segment: &Segment) -> Result<(), LogError> {
        let (record, record_len) = encode_record(segment)?;
        if record_len > self.storage.sector_size() - SECTOR_HEADER_LEN {
            return Err(LogError::Full);
        }
        loop {
            if let Some(sector) = self.current {
                if self.used + record_len <= self.storage.sector_size() {
                    if !self.storage.write(sector, self.used, &record[..record_len]) {
                        // A failed write may leave a partial record, so this sector is sealed.
                        self.current = None;
                        return Err(LogError::Storage);
                    }
                    self.used += record_len;
                    return Ok(());
                }
            }
            self.open_next_sector()?;
        }
    }

    /// Calls `visit` for every stored segment, oldest first. Returns how many were visited.
    pub fn replay(&self, mut visit: impl FnMut(Segment)) -> usize {
        let mut count = 0;
        let mut after = None;
        while let Some((seq, sector)) = next_in_order(&self.storage, after) {
            let mut offset = SECTOR_HEADER_LEN;
            while let Some((segment, next)) = read_record(&self.storage, sector, offset) {
                visit(segment);
                count += 1;
                offset = next;
            }
            after = Some(seq);
        }
        count
    }

    /// Erases the oldest sector, freeing its space. Call this only after the
    /// phone has synced every segment in that sector. Returns `false` when no
    /// sector holds data.
    pub fn retire_oldest(&mut self) -> Result<bool, LogError> {
        let Some((_, sector)) = next_in_order(&self.storage, None) else {
            return Ok(false);
        };
        if self.current == Some(sector) {
            self.current = None;
        }
        if !self.storage.erase(sector) {
            return Err(LogError::Storage);
        }
        Ok(true)
    }

    /// Number of segments in the oldest sector that holds data. Zero when the log is empty.
    pub fn oldest_sector_segments(&self) -> usize {
        let Some((_, sector)) = next_in_order(&self.storage, None) else {
            return 0;
        };
        let mut offset = SECTOR_HEADER_LEN;
        let mut count = 0;
        while let Some((_, next)) = read_record(&self.storage, sector, offset) {
            count += 1;
            offset = next;
        }
        count
    }

    /// Gives back the storage, for example to reopen the log after a simulated reset.
    pub fn into_storage(self) -> S {
        self.storage
    }

    /// Bytes in each sector.
    pub fn sector_size(&self) -> usize {
        self.storage.sector_size()
    }

    /// Number of sectors in the log, whether or not they hold data.
    pub fn sector_count(&self) -> usize {
        self.storage.sector_count()
    }

    /// Number of sectors that hold data.
    pub fn live_sectors(&self) -> usize {
        (0..self.storage.sector_count())
            .filter(|&sector| read_header(&self.storage, sector).is_some())
            .count()
    }

    /// Erases the next free sector in round-robin order and starts writing to it.
    fn open_next_sector(&mut self) -> Result<(), LogError> {
        let count = self.storage.sector_count();
        let start = self.last_opened.map_or(0, |sector| (sector + 1) % count);
        self.current = None;
        let sector = (0..count)
            .map(|step| (start + step) % count)
            .find(|&sector| read_header(&self.storage, sector).is_none())
            .ok_or(LogError::Full)?;

        if !self.storage.erase(sector) {
            return Err(LogError::Storage);
        }
        let header = sector_header(self.next_seq);
        if !self.storage.write(sector, 0, &header) {
            return Err(LogError::Storage);
        }
        self.current = Some(sector);
        self.last_opened = Some(sector);
        self.used = SECTOR_HEADER_LEN;
        self.next_seq = self.next_seq.wrapping_add(1);
        Ok(())
    }
}

/// Builds a sector header: magic, sequence number and a CRC over both.
fn sector_header(seq: u32) -> [u8; SECTOR_HEADER_LEN] {
    let mut header = [0u8; SECTOR_HEADER_LEN];
    header[0] = SECTOR_MAGIC;
    header[1..5].copy_from_slice(&seq.to_le_bytes());
    let crc = log::crc32(&header[..5]);
    header[5..9].copy_from_slice(&crc.to_le_bytes());
    header
}

/// The sequence number in a sector's header, or `None` if the sector is free or damaged.
fn read_header<S: SectorStorage>(storage: &S, sector: usize) -> Option<u32> {
    let mut header = [0u8; SECTOR_HEADER_LEN];
    if !storage.read(sector, 0, &mut header) || header[0] != SECTOR_MAGIC {
        return None;
    }
    let stored = u32::from_le_bytes([header[5], header[6], header[7], header[8]]);
    if stored != log::crc32(&header[..5]) {
        return None;
    }
    Some(u32::from_le_bytes([
        header[1], header[2], header[3], header[4],
    ]))
}

/// The sector with the smallest sequence number after `after`, if any.
fn next_in_order<S: SectorStorage>(storage: &S, after: Option<u32>) -> Option<(u32, usize)> {
    let mut best: Option<(u32, usize)> = None;
    for sector in 0..storage.sector_count() {
        if let Some(seq) = read_header(storage, sector) {
            if after.is_some_and(|prev| seq <= prev) {
                continue;
            }
            if best.is_none_or(|(least, _)| seq < least) {
                best = Some((seq, sector));
            }
        }
    }
    best
}

/// Reads the record at `offset` within `sector`. Returns the segment and the offset after it.
fn read_record<S: SectorStorage>(
    storage: &S,
    sector: usize,
    offset: usize,
) -> Option<(Segment, usize)> {
    let mut header = [0u8; log::HEADER_LEN];
    if !storage.read(sector, offset, &mut header) || header[0] != log::MAGIC {
        return None;
    }
    let payload_len = u16::from_le_bytes([header[1], header[2]]) as usize;
    if payload_len == 0 || payload_len > log::MAX_PAYLOAD {
        return None;
    }
    let mut payload = [0u8; log::MAX_PAYLOAD];
    let payload = &mut payload[..payload_len];
    if !storage.read(sector, offset + log::HEADER_LEN, payload) {
        return None;
    }
    let stored_crc = u32::from_le_bytes([header[3], header[4], header[5], header[6]]);
    if stored_crc != log::crc32(payload) {
        return None;
    }
    let segment = decode_segment(payload)?;
    Some((segment, offset + log::HEADER_LEN + payload_len))
}

/// Where the valid records in `sector` end, and whether the rest of the sector is erased.
fn scan_end<S: SectorStorage>(storage: &S, sector: usize) -> (usize, bool) {
    let mut offset = SECTOR_HEADER_LEN;
    while let Some((_, next)) = read_record(storage, sector, offset) {
        offset = next;
    }
    (offset, tail_is_erased(storage, sector, offset))
}

/// True when every byte from `from` to the end of `sector` is `0xFF`.
fn tail_is_erased<S: SectorStorage>(storage: &S, sector: usize, from: usize) -> bool {
    const CHUNK: usize = 16;
    let size = storage.sector_size();
    let mut offset = from;
    while offset < size {
        let len = CHUNK.min(size - offset);
        let mut chunk = [0u8; CHUNK];
        let chunk = &mut chunk[..len];
        if !storage.read(sector, offset, chunk) || chunk.iter().any(|&byte| byte != 0xff) {
            return false;
        }
        offset += len;
    }
    true
}

/// Encodes one segment as a complete record. Returns the buffer and the number of bytes used.
pub(crate) fn encode_record(
    segment: &Segment,
) -> Result<([u8; log::HEADER_LEN + log::MAX_PAYLOAD], usize), LogError> {
    let mut payload = [0u8; log::MAX_PAYLOAD];
    let payload_len = encode_segment(segment, &mut payload)?;
    let mut record = [0u8; log::HEADER_LEN + log::MAX_PAYLOAD];
    record[0] = log::MAGIC;
    record[1..3].copy_from_slice(&(payload_len as u16).to_le_bytes());
    record[3..7].copy_from_slice(&log::crc32(&payload[..payload_len]).to_le_bytes());
    record[log::HEADER_LEN..log::HEADER_LEN + payload_len].copy_from_slice(&payload[..payload_len]);
    Ok((record, log::HEADER_LEN + payload_len))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// RAM-backed sectors that start erased, as flash does. It can cut power
    /// after a set number of operations. The operation that is cut writes only
    /// part of its data, and every later operation fails.
    pub(crate) struct RamSectors {
        pub(crate) bytes: std::vec::Vec<u8>,
        sector_size: usize,
        pub(crate) ops_left: usize,
        pub(crate) dead: bool,
        pub(crate) erases: std::vec::Vec<u32>,
    }

    impl RamSectors {
        pub(crate) fn new(sectors: usize, sector_size: usize) -> Self {
            Self {
                bytes: std::vec![0xff; sectors * sector_size],
                sector_size,
                ops_left: usize::MAX,
                dead: false,
                erases: std::vec![0; sectors],
            }
        }

        /// Spends one operation. Returns `false` when power is already gone.
        fn spend(&mut self) -> bool {
            if self.dead || self.ops_left == 0 {
                self.dead = true;
                return false;
            }
            self.ops_left -= 1;
            true
        }
    }

    impl SectorStorage for RamSectors {
        fn sector_count(&self) -> usize {
            self.bytes.len() / self.sector_size
        }
        fn sector_size(&self) -> usize {
            self.sector_size
        }
        fn read(&self, sector: usize, offset: usize, buf: &mut [u8]) -> bool {
            if offset + buf.len() > self.sector_size {
                return false;
            }
            let start = sector * self.sector_size + offset;
            buf.copy_from_slice(&self.bytes[start..start + buf.len()]);
            true
        }
        fn write(&mut self, sector: usize, offset: usize, data: &[u8]) -> bool {
            if offset + data.len() > self.sector_size || !self.spend() {
                return false;
            }
            let start = sector * self.sector_size + offset;
            if self.ops_left == 0 {
                // Power is cut during this write: half of the data lands, then nothing else.
                let landed = data.len() / 2;
                and_into(&mut self.bytes[start..start + landed], &data[..landed]);
                self.dead = true;
                return false;
            }
            and_into(&mut self.bytes[start..start + data.len()], data);
            true
        }
        fn erase(&mut self, sector: usize) -> bool {
            if !self.spend() {
                return false;
            }
            let start = sector * self.sector_size;
            self.bytes[start..start + self.sector_size].fill(0xff);
            self.erases[sector] += 1;
            true
        }
    }

    /// NOR flash can only clear bits: a write ANDs with what is already there.
    fn and_into(target: &mut [u8], data: &[u8]) {
        for (byte, new) in target.iter_mut().zip(data) {
            *byte &= new;
        }
    }

    fn segment(start_ms: u64) -> Segment {
        Segment {
            start_ms,
            end_ms: start_ms + 60_000,
            distance_m: start_ms as f64 * 10.0,
        }
    }

    fn collect<S: SectorStorage>(log: &SectorLog<S>) -> std::vec::Vec<Segment> {
        let mut out = std::vec::Vec::new();
        log.replay(|s| out.push(s));
        out
    }

    #[test]
    fn segments_replay_in_order_across_sectors() {
        // 128-byte sectors hold a few records each, so this spans several sectors.
        let mut log = SectorLog::open(RamSectors::new(4, 128));
        let segments: std::vec::Vec<Segment> = (1..=8).map(segment).collect();
        for s in &segments {
            log.append(s).unwrap();
        }
        assert!(log.live_sectors() > 1);
        assert_eq!(collect(&log), segments);
    }

    #[test]
    fn reopening_keeps_every_record() {
        let mut log = SectorLog::open(RamSectors::new(4, 128));
        for i in 1..=5 {
            log.append(&segment(i)).unwrap();
        }
        let reopened = SectorLog::open(log.storage);
        assert_eq!(collect(&reopened).len(), 5);
    }

    #[test]
    fn writes_are_spread_round_robin_across_sectors() {
        let mut log = SectorLog::open(RamSectors::new(4, 128));
        // Many more records than sectors, then retire and refill, several times over.
        for round in 0..6u64 {
            for i in 0..6 {
                if log.append(&segment(round * 10 + i)).is_err() {
                    log.retire_oldest().unwrap();
                    log.append(&segment(round * 10 + i)).unwrap();
                }
            }
        }
        let erases = &log.storage.erases;
        let most = *erases.iter().max().unwrap();
        let least = *erases.iter().min().unwrap();
        assert!(most - least <= 1, "erase counts {erases:?}");
    }

    #[test]
    fn retiring_removes_the_oldest_sector_only() {
        let mut log = SectorLog::open(RamSectors::new(4, 128));
        for i in 1..=8 {
            log.append(&segment(i)).unwrap();
        }
        let before = collect(&log);
        assert!(log.retire_oldest().unwrap());
        let after = collect(&log);
        assert!(after.len() < before.len());
        assert_eq!(after.as_slice(), &before[before.len() - after.len()..]);
    }

    #[test]
    fn a_full_log_refuses_until_a_sector_is_retired() {
        let mut log = SectorLog::open(RamSectors::new(2, 128));
        let mut appended = 0;
        while log.append(&segment(appended + 1)).is_ok() {
            appended += 1;
        }
        assert!(appended > 0);
        assert_eq!(log.append(&segment(999)), Err(LogError::Full));
        assert!(log.retire_oldest().unwrap());
        assert!(log.append(&segment(999)).is_ok());
    }

    #[test]
    fn power_cut_at_every_point_never_corrupts_earlier_records() {
        // Cut power after every possible number of operations, in a run of
        // appends and one retirement. Reopening must always give a clean prefix
        // of what was written, and every append that reported success must survive.
        let attempts: std::vec::Vec<Segment> = (1..=10).map(segment).collect();
        for cut_after in 0..120 {
            let mut storage = RamSectors::new(4, 128);
            storage.ops_left = cut_after;
            let mut log = SectorLog::open(storage);
            let mut acknowledged = std::vec::Vec::new();
            for s in &attempts {
                if log.append(s).is_ok() {
                    acknowledged.push(*s);
                }
            }

            let mut storage = log.storage;
            storage.dead = false;
            storage.ops_left = usize::MAX;
            let mut reopened = SectorLog::open(storage);
            let replayed = collect(&reopened);

            // Every replayed segment was written, and they come out in order.
            assert!(
                replayed.iter().all(|s| attempts.contains(s)),
                "cut {cut_after}"
            );
            assert!(
                replayed.windows(2).all(|w| w[0].start_ms < w[1].start_ms),
                "cut {cut_after}"
            );
            // Every acknowledged append is still there.
            for s in &acknowledged {
                assert!(replayed.contains(s), "cut {cut_after} lost {s:?}");
            }
            // The log still accepts writes after recovery, unless it is simply full.
            match reopened.append(&segment(500)) {
                Ok(()) => assert!(
                    collect(&reopened).contains(&segment(500)),
                    "cut {cut_after}"
                ),
                Err(error) => assert_eq!(error, LogError::Full, "cut {cut_after}"),
            }
        }
    }

    #[test]
    fn an_empty_device_has_nothing_to_replay() {
        let log = SectorLog::open(RamSectors::new(3, 128));
        assert!(collect(&log).is_empty());
        assert_eq!(log.live_sectors(), 0);
    }
}
