// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Sends stored segments over BLE, and frees flash only after the phone has them.
//!
//! A [`TransferSession`] starts when the phone requests segments. It reads the
//! flash log oldest first, in batches of `K` records, and writes chunks with
//! [`ChunkCursor`]. Chunks carry the same record bytes the log holds, so the
//! phone checks them against the same CRC.
//!
//! Segments are numbered by their position in the transfer, from 0. The phone
//! acknowledges a count, meaning it holds the first `count` segments. The
//! session frees a sector only when every segment in it is covered by that
//! count. A partly covered sector stays, and its acknowledged segments are sent
//! again in the next transfer. The phone drops those duplicates.
//!
//! A dropped connection therefore loses nothing: whatever was not acknowledged
//! is still in the log.
//!
//! The log is only read during a transfer, so a transfer of an empty log sends
//! no chunks. The phone learns this from the used-bytes count in the device info.

use crate::flash::{encode_record, SectorLog, SectorStorage};
use crate::log::{LogError, HEADER_LEN, MAX_PAYLOAD};
use crate::transfer::{ChunkCursor, TransferError};

/// Largest encoded record: header and the longest payload.
const RECORD_MAX: usize = HEADER_LEN + MAX_PAYLOAD;

/// Why a transfer step or an acknowledgement was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// The chunk encoder refused, for example because the buffer is too small.
    Transfer(TransferError),
    /// The phone acknowledged more segments than have been sent.
    AckAheadOfSent {
        /// Segments sent so far in this transfer.
        sent: u32,
    },
    /// The flash log failed.
    Storage(LogError),
}

impl From<TransferError> for SessionError {
    fn from(error: TransferError) -> Self {
        Self::Transfer(error)
    }
}

/// One transfer of the flash log to the phone.
#[derive(Debug)]
pub struct TransferSession<const K: usize> {
    cursor: ChunkCursor,
    records: [[u8; RECORD_MAX]; K],
    lens: [usize; K],
    count: usize,
    /// Transfer index of the first record in the current batch.
    batch_start: u32,
    /// Transfer index of the first segment still in the log. Earlier segments were released.
    base: u32,
    /// Whether the log holds records after the current batch.
    more: bool,
}

impl<const K: usize> TransferSession<K> {
    /// Starts a transfer at `mtu`, from the oldest segment in `log`.
    pub fn start<S: SectorStorage>(log: &SectorLog<S>, mtu: u16) -> Result<Self, SessionError> {
        let mut session = Self {
            cursor: ChunkCursor::new(mtu)?,
            records: [[0; RECORD_MAX]; K],
            lens: [0; K],
            count: 0,
            batch_start: 0,
            base: 0,
            more: false,
        };
        session.refill(log)?;
        Ok(session)
    }

    /// Segments of this transfer that have been sent in full.
    pub fn sent(&self) -> u32 {
        self.batch_start + self.cursor.completed() as u32
    }

    /// Writes the next chunk into `out`. Returns its length, or `None` when the transfer is complete.
    pub fn next_chunk<S: SectorStorage>(
        &mut self,
        log: &SectorLog<S>,
        out: &mut [u8],
    ) -> Result<Option<usize>, SessionError> {
        loop {
            let mut refs: [&[u8]; K] = [&[]; K];
            for (i, slot) in refs.iter_mut().enumerate().take(self.count) {
                *slot = &self.records[i][..self.lens[i]];
            }
            let chunk = self
                .cursor
                .next_chunk(&refs[..self.count], !self.more, out)?;
            if chunk.is_some() {
                return Ok(chunk);
            }
            if !self.more {
                return Ok(None);
            }
            self.batch_start += self.count as u32;
            self.cursor.restart_batch();
            self.refill(log)?;
            if self.count == 0 {
                return Ok(None);
            }
        }
    }

    /// Records that the phone holds the first `count` segments of this transfer.
    /// Frees each oldest sector whose segments are all covered. Returns how many sectors were freed.
    pub fn acknowledge<S: SectorStorage>(
        &mut self,
        log: &mut SectorLog<S>,
        count: u32,
    ) -> Result<usize, SessionError> {
        let sent = self.sent();
        if count > sent {
            return Err(SessionError::AckAheadOfSent { sent });
        }
        let mut freed = 0;
        loop {
            let segments = log.oldest_sector_segments() as u32;
            if segments == 0 || self.base + segments > count {
                break;
            }
            match log.retire_oldest() {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => return Err(SessionError::Storage(error)),
            }
            self.base += segments;
            freed += 1;
        }
        Ok(freed)
    }

    /// Loads the next batch of up to `K` records, starting at the current batch position.
    fn refill<S: SectorStorage>(&mut self, log: &SectorLog<S>) -> Result<(), SessionError> {
        let start = (self.batch_start - self.base) as usize;
        self.count = 0;
        self.more = false;
        let mut index = 0;
        let mut failed = false;
        log.replay(|segment| {
            if index >= start {
                if self.count < K {
                    match encode_record(&segment) {
                        Ok((record, len)) => {
                            self.records[self.count][..len].copy_from_slice(&record[..len]);
                            self.lens[self.count] = len;
                            self.count += 1;
                        }
                        Err(_) => failed = true,
                    }
                } else {
                    self.more = true;
                }
            }
            index += 1;
        });
        if failed {
            return Err(SessionError::Storage(LogError::Storage));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::tests::RamSectors;
    use crate::log::decode_segment;
    use crate::transfer::Reassembler;
    use tpt_ruc_core::Segment;

    const MTU: u16 = 247;

    fn segment(i: u64) -> Segment {
        Segment {
            start_ms: 1_000 * i,
            end_ms: 1_000 * i + 900,
            distance_m: i as f64 + 0.5,
        }
    }

    /// A log of `count` segments over `sectors` sectors of `sector_size` bytes.
    fn filled(count: u64, sectors: usize, sector_size: usize) -> SectorLog<RamSectors> {
        let mut log = SectorLog::open(RamSectors::new(sectors, sector_size));
        for i in 0..count {
            log.append(&segment(i)).unwrap();
        }
        log
    }

    /// Sends every remaining chunk and returns the segments the phone would store.
    fn send_all<const K: usize, S: SectorStorage>(
        session: &mut TransferSession<K>,
        log: &SectorLog<S>,
    ) -> Vec<Segment> {
        let mut reassembler: Reassembler<64> = Reassembler::new();
        let mut received = Vec::new();
        let mut buf = [0u8; MTU as usize];
        while let Some(len) = session.next_chunk(log, &mut buf).unwrap() {
            if let Some(record) = reassembler.push(&buf[..len]).unwrap() {
                received.push(decode_segment(&record[HEADER_LEN..]).unwrap());
            }
        }
        if !received.is_empty() {
            assert!(
                reassembler.finished(),
                "the last chunk carries the transfer end"
            );
        }
        received
    }

    #[test]
    fn every_segment_is_sent_once_in_order_across_batches() {
        let log = filled(15, 4, 256);
        let mut session: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        let received = send_all(&mut session, &log);
        let expected: Vec<Segment> = (0..15).map(segment).collect();
        assert_eq!(received, expected);
        assert_eq!(session.sent(), 15);
    }

    #[test]
    fn an_empty_log_sends_no_chunks() {
        let log = filled(0, 2, 256);
        let mut session: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        assert!(send_all(&mut session, &log).is_empty());
    }

    #[test]
    fn a_short_buffer_is_reported_and_the_chunk_can_be_retried() {
        let log = filled(2, 2, 256);
        let mut session: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        let mut small = [0u8; 4];
        assert_eq!(
            session.next_chunk(&log, &mut small),
            Err(SessionError::Transfer(TransferError::BufferTooSmall))
        );
        let expected: Vec<Segment> = (0..2).map(segment).collect();
        assert_eq!(send_all(&mut session, &log), expected);
    }

    #[test]
    fn acks_free_only_whole_sectors() {
        let mut log = filled(15, 4, 256);
        let first = log.oldest_sector_segments() as u32;
        let sectors_before = log.live_sectors();
        let mut session: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        send_all(&mut session, &log);

        // One short of the first sector: nothing may be freed.
        assert_eq!(session.acknowledge(&mut log, first - 1), Ok(0));
        assert_eq!(log.live_sectors(), sectors_before);

        // The whole first sector: it is freed, and the rest stays.
        assert_eq!(session.acknowledge(&mut log, first), Ok(1));
        assert_eq!(log.live_sectors(), sectors_before - 1);
    }

    #[test]
    fn unacknowledged_segments_are_sent_again_in_the_next_transfer() {
        let mut log = filled(15, 4, 256);
        let first = log.oldest_sector_segments() as u32;
        let mut session: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        send_all(&mut session, &log);
        // The phone stored part of the first sector, then the connection dropped.
        session.acknowledge(&mut log, first - 1).unwrap();

        let mut again: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        let received = send_all(&mut again, &log);
        let expected: Vec<Segment> = (0..15).map(segment).collect();
        assert_eq!(received, expected, "nothing acknowledged was lost");
    }

    #[test]
    fn acknowledging_more_than_was_sent_is_refused() {
        let mut log = filled(5, 4, 256);
        let mut session: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        send_all(&mut session, &log);
        assert_eq!(
            session.acknowledge(&mut log, 6),
            Err(SessionError::AckAheadOfSent { sent: 5 })
        );
    }

    #[test]
    fn a_later_transfer_after_acks_starts_at_the_oldest_kept_segment() {
        let mut log = filled(15, 4, 256);
        let first = log.oldest_sector_segments() as u32;
        let mut session: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        send_all(&mut session, &log);
        session.acknowledge(&mut log, first).unwrap();

        let mut again: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
        let received = send_all(&mut again, &log);
        let expected: Vec<Segment> = (u64::from(first)..15).map(segment).collect();
        assert_eq!(received, expected);
    }

    #[test]
    fn power_cut_during_a_release_never_loses_unacknowledged_segments() {
        // Cut power after every possible number of flash operations during the
        // acknowledgement. Segments the phone has not acknowledged must survive every cut.
        let acked = 9;
        for cut_after in 0..40 {
            let log = filled(15, 4, 256);
            let mut storage = log.into_storage();
            storage.ops_left = cut_after;
            let mut log = SectorLog::open(storage);

            let mut session: TransferSession<3> = TransferSession::start(&log, MTU).unwrap();
            send_all(&mut session, &log);
            let _ = session.acknowledge(&mut log, acked);

            let mut storage = log.into_storage();
            storage.dead = false;
            storage.ops_left = usize::MAX;
            let reopened = SectorLog::open(storage);
            let mut replayed = Vec::new();
            reopened.replay(|s| replayed.push(s));

            let unacked: Vec<Segment> = (acked as u64..15).map(segment).collect();
            assert!(
                replayed.ends_with(&unacked),
                "cut {cut_after}: unacknowledged segments lost, got {replayed:?}"
            );
        }
    }
}
