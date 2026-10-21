// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Append-only trip segment log for flash storage.
//!
//! The log stores each finished segment as one record:
//!
//! `[magic: u8 = 0xA5][payload_len: u16 LE][crc32: u32 LE][payload]`
//!
//! The payload is a MessagePack array `[start_ms, end_ms, distance_m]`.
//!
//! Records are only ever appended. Erased flash reads as `0xFF`, so unwritten
//! space looks like the end of the log. A record cut short by power loss fails
//! its CRC, and replay stops there. Earlier records survive.
//!
//! The hardware driver is behind [`LogStorage`]. This module is tested on the
//! host with a RAM-backed store. The ESP flash driver is not written yet.

use rmp::decode;
use rmp::encode;
use tpt_ruc_core::Segment;

/// First byte of every record.
pub const MAGIC: u8 = 0xA5;

/// Bytes before the payload: magic, length and CRC.
pub const HEADER_LEN: usize = 1 + 2 + 4;

/// Largest payload this log writes. A segment needs about 25 bytes.
pub const MAX_PAYLOAD: usize = 32;

/// Byte-addressed storage the log writes to. Flash drivers implement this.
pub trait LogStorage {
    /// Total capacity in bytes.
    fn capacity(&self) -> usize;
    /// Fills `buf` from `offset`. Returns `false` if the range is out of bounds.
    fn read(&self, offset: usize, buf: &mut [u8]) -> bool;
    /// Writes `data` at `offset`. Returns `false` if the range is out of bounds.
    /// Writes may only clear bits, as on NOR flash. The log only writes erased space.
    fn write(&mut self, offset: usize, data: &[u8]) -> bool;
}

/// Why a segment could not be appended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogError {
    /// No room is left for another record.
    Full,
    /// The storage refused the write.
    Storage,
}

/// Append-only log of trip segments over a [`LogStorage`].
#[derive(Debug, Clone, Copy)]
pub struct SegmentLog<S> {
    storage: S,
    /// Offset just past the last valid record.
    end: usize,
}

impl<S: LogStorage> SegmentLog<S> {
    /// Opens the log, scanning to find the end of the valid records.
    pub fn open(storage: S) -> Self {
        let mut log = Self { storage, end: 0 };
        let mut offset = 0;
        while let Some((_, next)) = log.record_at(offset) {
            offset = next;
        }
        log.end = offset;
        log
    }

    /// Appends one segment.
    pub fn append(&mut self, segment: &Segment) -> Result<(), LogError> {
        let mut payload = [0u8; MAX_PAYLOAD];
        let payload_len = encode_segment(segment, &mut payload)?;
        let record_len = HEADER_LEN + payload_len;
        if self.end + record_len > self.storage.capacity() {
            return Err(LogError::Full);
        }

        let mut record = [0u8; HEADER_LEN + MAX_PAYLOAD];
        record[0] = MAGIC;
        record[1..3].copy_from_slice(&(payload_len as u16).to_le_bytes());
        record[3..7].copy_from_slice(&crc32(&payload[..payload_len]).to_le_bytes());
        record[HEADER_LEN..record_len].copy_from_slice(&payload[..payload_len]);

        if !self.storage.write(self.end, &record[..record_len]) {
            return Err(LogError::Storage);
        }
        self.end += record_len;
        Ok(())
    }

    /// Calls `visit` for each valid segment, oldest first. Returns how many were visited.
    pub fn replay(&self, mut visit: impl FnMut(Segment)) -> usize {
        let mut offset = 0;
        let mut count = 0;
        while let Some((segment, next)) = self.record_at(offset) {
            visit(segment);
            count += 1;
            offset = next;
        }
        count
    }

    /// Bytes used by valid records.
    pub fn used(&self) -> usize {
        self.end
    }

    /// Returns the segment at `offset` and the offset of the next record.
    /// Returns `None` at erased space, a damaged record, or the end of storage.
    fn record_at(&self, offset: usize) -> Option<(Segment, usize)> {
        let mut header = [0u8; HEADER_LEN];
        if !self.storage.read(offset, &mut header) || header[0] != MAGIC {
            return None;
        }
        let payload_len = u16::from_le_bytes([header[1], header[2]]) as usize;
        if payload_len == 0 || payload_len > MAX_PAYLOAD {
            return None;
        }
        let mut payload = [0u8; MAX_PAYLOAD];
        let payload = &mut payload[..payload_len];
        if !self.storage.read(offset + HEADER_LEN, payload) {
            return None;
        }
        let stored_crc = u32::from_le_bytes([header[3], header[4], header[5], header[6]]);
        if stored_crc != crc32(payload) {
            return None;
        }
        let segment = decode_segment(payload)?;
        Some((segment, offset + HEADER_LEN + payload_len))
    }
}

/// Encodes a segment as `[start_ms, end_ms, distance_m]`. Returns the bytes written.
pub(crate) fn encode_segment(segment: &Segment, buf: &mut [u8]) -> Result<usize, LogError> {
    let total = buf.len();
    let mut cursor: &mut [u8] = buf;
    encode::write_array_len(&mut cursor, 3).map_err(|_| LogError::Storage)?;
    encode::write_uint(&mut cursor, segment.start_ms).map_err(|_| LogError::Storage)?;
    encode::write_uint(&mut cursor, segment.end_ms).map_err(|_| LogError::Storage)?;
    encode::write_f64(&mut cursor, segment.distance_m).map_err(|_| LogError::Storage)?;
    Ok(total - cursor.len())
}

/// Decodes a payload written by [`encode_segment`]. `None` if it is malformed.
pub(crate) fn decode_segment(payload: &[u8]) -> Option<Segment> {
    let mut reader = payload;
    if decode::read_array_len(&mut reader).ok()? != 3 {
        return None;
    }
    let start_ms: u64 = decode::read_int(&mut reader).ok()?;
    let end_ms: u64 = decode::read_int(&mut reader).ok()?;
    let distance_m = decode::read_f64(&mut reader).ok()?;
    Some(Segment {
        start_ms,
        end_ms,
        distance_m,
    })
}

/// CRC-32 (IEEE, reflected), the checksum stored in every log record.
/// Computed bit by bit, so it needs no lookup table.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RAM-backed storage that starts erased (0xFF), like flash.
    struct RamFlash {
        bytes: std::vec::Vec<u8>,
    }

    impl RamFlash {
        fn new(size: usize) -> Self {
            Self {
                bytes: std::vec![0xff; size],
            }
        }
    }

    impl LogStorage for RamFlash {
        fn capacity(&self) -> usize {
            self.bytes.len()
        }
        fn read(&self, offset: usize, buf: &mut [u8]) -> bool {
            match self.bytes.get(offset..offset + buf.len()) {
                Some(src) => {
                    buf.copy_from_slice(src);
                    true
                }
                None => false,
            }
        }
        fn write(&mut self, offset: usize, data: &[u8]) -> bool {
            match self.bytes.get_mut(offset..offset + data.len()) {
                Some(dst) => {
                    dst.copy_from_slice(data);
                    true
                }
                None => false,
            }
        }
    }

    fn segment(start_ms: u64, distance_m: f64) -> Segment {
        Segment {
            start_ms,
            end_ms: start_ms + 60_000,
            distance_m,
        }
    }

    fn collect(log: &SegmentLog<RamFlash>) -> std::vec::Vec<Segment> {
        let mut out = std::vec::Vec::new();
        log.replay(|s| out.push(s));
        out
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        // The published CRC-32 check value for the ASCII string "123456789".
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn appended_segments_replay_in_order() {
        let mut log = SegmentLog::open(RamFlash::new(1024));
        log.append(&segment(1, 100.5)).unwrap();
        log.append(&segment(2, 250.0)).unwrap();
        assert_eq!(collect(&log), vec![segment(1, 100.5), segment(2, 250.0)]);
    }

    #[test]
    fn reopening_finds_the_existing_records() {
        let mut log = SegmentLog::open(RamFlash::new(1024));
        log.append(&segment(1, 10.0)).unwrap();
        log.append(&segment(2, 20.0)).unwrap();
        let used = log.used();

        // Reopen on the same bytes, as after a reset.
        let reopened = SegmentLog::open(log.storage);
        assert_eq!(reopened.used(), used);
        assert_eq!(collect(&reopened).len(), 2);
    }

    #[test]
    fn a_damaged_record_ends_the_replay_but_keeps_earlier_ones() {
        let mut log = SegmentLog::open(RamFlash::new(1024));
        log.append(&segment(1, 10.0)).unwrap();
        let second_at = log.used();
        log.append(&segment(2, 20.0)).unwrap();
        log.append(&segment(3, 30.0)).unwrap();

        // Corrupt one payload byte of the second record.
        log.storage.bytes[second_at + HEADER_LEN] ^= 0x01;

        assert_eq!(collect(&log), vec![segment(1, 10.0)]);
    }

    #[test]
    fn a_record_cut_short_by_power_loss_is_ignored() {
        let mut log = SegmentLog::open(RamFlash::new(1024));
        log.append(&segment(1, 10.0)).unwrap();
        let second_at = log.used();
        log.append(&segment(2, 20.0)).unwrap();

        // Erase the tail of the second record, as an interrupted write would leave it.
        for byte in &mut log.storage.bytes[second_at + HEADER_LEN + 2..] {
            *byte = 0xff;
        }

        let reopened = SegmentLog::open(log.storage);
        assert_eq!(collect(&reopened), vec![segment(1, 10.0)]);
        assert_eq!(reopened.used(), second_at);
    }

    #[test]
    fn a_full_log_refuses_more_records() {
        // Room for exactly one record of this size, so the second append must fail.
        let record_len = HEADER_LEN + 16;
        let mut log = SegmentLog::open(RamFlash::new(record_len));
        log.append(&segment(1, 1.0)).unwrap();
        assert_eq!(log.append(&segment(2, 2.0)), Err(LogError::Full));
        assert_eq!(collect(&log), vec![segment(1, 1.0)]);
    }

    #[test]
    fn erased_storage_is_an_empty_log() {
        let log = SegmentLog::open(RamFlash::new(64));
        assert_eq!(log.used(), 0);
        assert!(collect(&log).is_empty());
    }
}
