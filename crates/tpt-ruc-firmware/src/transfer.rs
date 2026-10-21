// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Segment transfer over BLE: chunking the segment log for notifications.
//!
//! The framing is version 1 of `docs/ble-design.md`. Each notification is one
//! chunk:
//!
//! `[seq: u16 LE][flags: u8][data...]`
//!
//! - `seq` counts chunks within one transfer, from 0.
//! - Bit 0 of `flags` is set on the last chunk of a segment, and bit 1 on the
//!   last chunk of the transfer.
//! - `data` is the segment's log record bytes, unchanged, so the phone checks
//!   them against the same CRC-32 as the log.
//!
//! A chunk never spans two segments. [`ChunkEncoder`] writes chunks and
//! [`Reassembler`] reads them back in the same way as the PWA's
//! `SegmentAssembler` in `pwa/lib/ble.ts`. Both are allocation-free.

use crate::log::{crc32, HEADER_LEN, MAGIC};

/// Bytes at the start of every chunk: `seq` and `flags`.
pub const CHUNK_HEADER_LEN: usize = 3;
/// ATT header the stack adds to every notification.
pub const ATT_HEADER_LEN: usize = 3;
/// MTU a BLE link starts with before the phone asks for more.
pub const DEFAULT_MTU: u16 = 23;
/// Set on the last chunk of one segment.
pub const FLAG_END_OF_SEGMENT: u8 = 0x01;
/// Set on the last chunk of the transfer.
pub const FLAG_END_OF_TRANSFER: u8 = 0x02;

/// Why a transfer could not be encoded or decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferError {
    /// The MTU leaves no room for chunk data after the headers.
    MtuTooSmall,
    /// The output buffer cannot hold the next chunk.
    BufferTooSmall,
    /// The transfer has more chunks than a 16-bit sequence number can count.
    TransferTooLong,
    /// A chunk arrived with a sequence number other than the one expected.
    OutOfOrder {
        /// The sequence number the reassembler expected next.
        expected: u16,
        /// The sequence number that arrived.
        got: u16,
    },
    /// A chunk arrived after the transfer had ended.
    AfterEnd,
    /// A chunk is shorter than its header.
    ChunkTooShort,
    /// A segment is larger than the reassembler's buffer.
    RecordTooLarge,
    /// The transfer ended in the middle of a segment.
    EndedMidSegment,
    /// The record does not start with the log magic byte.
    BadMagic,
    /// The record's length field does not match its bytes.
    BadLength,
    /// The record's CRC does not match its payload.
    BadCrc,
}

/// Chunk data bytes that fit in one notification at `mtu`. Returns `None` when
/// the MTU leaves no room for data.
pub fn chunk_data_len(mtu: u16) -> Option<usize> {
    usize::from(mtu)
        .checked_sub(ATT_HEADER_LEN + CHUNK_HEADER_LEN)
        .filter(|len| *len > 0)
}

/// Writes the chunks of a transfer, one notification at a time.
///
/// Call [`ChunkEncoder::next_chunk`] until it returns `Ok(None)`. A failed call
/// changes nothing, so the caller can retry it with a larger buffer.
#[derive(Debug)]
pub struct ChunkEncoder<'a> {
    records: &'a [&'a [u8]],
    record: usize,
    offset: usize,
    seq: u16,
    data_len: usize,
}

impl<'a> ChunkEncoder<'a> {
    /// Starts a transfer of `records`, one record per segment, oldest first.
    pub fn new(records: &'a [&'a [u8]], mtu: u16) -> Result<Self, TransferError> {
        let data_len = chunk_data_len(mtu).ok_or(TransferError::MtuTooSmall)?;
        Ok(Self {
            records,
            record: 0,
            offset: 0,
            seq: 0,
            data_len,
        })
    }

    /// Writes the next chunk into `out`. Returns its length, or `None` when
    /// the transfer is complete.
    pub fn next_chunk(&mut self, out: &mut [u8]) -> Result<Option<usize>, TransferError> {
        let Some(record) = self.records.get(self.record) else {
            return Ok(None);
        };
        let take = self.data_len.min(record.len() - self.offset);
        let total = CHUNK_HEADER_LEN + take;
        if out.len() < total {
            return Err(TransferError::BufferTooSmall);
        }

        let last_in_segment = self.offset + take == record.len();
        let last_in_transfer = last_in_segment && self.record + 1 == self.records.len();
        if last_in_transfer {
            // Nothing follows, so the sequence number need not advance further.
        } else if self.seq == u16::MAX {
            return Err(TransferError::TransferTooLong);
        }

        let mut flags = 0;
        if last_in_segment {
            flags |= FLAG_END_OF_SEGMENT;
        }
        if last_in_transfer {
            flags |= FLAG_END_OF_TRANSFER;
        }
        out[..2].copy_from_slice(&self.seq.to_le_bytes());
        out[2] = flags;
        out[CHUNK_HEADER_LEN..total].copy_from_slice(&record[self.offset..self.offset + take]);

        self.seq = self.seq.wrapping_add(1);
        self.offset += take;
        if last_in_segment {
            self.record += 1;
            self.offset = 0;
        }
        Ok(Some(total))
    }
}

/// Smallest record: magic, length and CRC, with an empty payload.
const MIN_RECORD_LEN: usize = HEADER_LEN;

/// Rebuilds segment records from chunks, checking their order and CRC.
///
/// `N` is the largest record the buffer can hold. The log writes at most
/// about 40 bytes per record, so 64 is ample.
#[derive(Debug)]
pub struct Reassembler<const N: usize> {
    expected_seq: u16,
    buf: [u8; N],
    len: usize,
    /// A record was returned by the last `push` and is still in `buf`.
    ready: bool,
    finished: bool,
}

impl<const N: usize> Default for Reassembler<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Reassembler<N> {
    /// Starts an empty transfer.
    pub const fn new() -> Self {
        Self {
            expected_seq: 0,
            buf: [0; N],
            len: 0,
            ready: false,
            finished: false,
        }
    }

    /// True once the last chunk of the transfer has been accepted.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Feeds one chunk. Returns the completed record when this chunk ends a
    /// segment. The slice stays valid until the next call.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Option<&[u8]>, TransferError> {
        if self.finished {
            return Err(TransferError::AfterEnd);
        }
        if chunk.len() < CHUNK_HEADER_LEN {
            return Err(TransferError::ChunkTooShort);
        }
        let seq = u16::from_le_bytes([chunk[0], chunk[1]]);
        if seq != self.expected_seq {
            return Err(TransferError::OutOfOrder {
                expected: self.expected_seq,
                got: seq,
            });
        }
        let flags = chunk[2];
        let data = &chunk[CHUNK_HEADER_LEN..];

        if self.ready {
            // The previous record has been handed out, so its space is free again.
            self.len = 0;
            self.ready = false;
        }
        if self.len + data.len() > N {
            return Err(TransferError::RecordTooLarge);
        }
        if flags & FLAG_END_OF_TRANSFER != 0 && flags & FLAG_END_OF_SEGMENT == 0 {
            return Err(TransferError::EndedMidSegment);
        }

        self.buf[self.len..self.len + data.len()].copy_from_slice(data);
        self.len += data.len();
        self.expected_seq = self.expected_seq.wrapping_add(1);

        let mut completed = None;
        if flags & FLAG_END_OF_SEGMENT != 0 {
            validate_record(&self.buf[..self.len])?;
            self.ready = true;
            completed = Some(self.len);
        }
        if flags & FLAG_END_OF_TRANSFER != 0 {
            self.finished = true;
        }
        Ok(completed.map(|len| &self.buf[..len]))
    }
}

/// Checks a record's magic, length and CRC. The same rules as the log replay.
fn validate_record(record: &[u8]) -> Result<(), TransferError> {
    if record.len() < MIN_RECORD_LEN {
        return Err(TransferError::BadLength);
    }
    if record[0] != MAGIC {
        return Err(TransferError::BadMagic);
    }
    let payload_len = u16::from_le_bytes([record[1], record[2]]) as usize;
    let payload = &record[HEADER_LEN..];
    if payload.len() != payload_len {
        return Err(TransferError::BadLength);
    }
    let stored = u32::from_le_bytes([record[3], record[4], record[5], record[6]]);
    if crc32(payload) != stored {
        return Err(TransferError::BadCrc);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log record for `payload`, built the same way the log writes it.
    fn record(payload: &[u8]) -> Vec<u8> {
        let mut out = vec![MAGIC];
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        out.extend_from_slice(&crc32(payload).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    /// Encodes every record into chunks, collecting each chunk as its own Vec.
    fn encode_all(records: &[&[u8]], mtu: u16) -> Vec<Vec<u8>> {
        let mut encoder = ChunkEncoder::new(records, mtu).unwrap();
        let mut chunks = Vec::new();
        loop {
            let mut buf = vec![0u8; usize::from(mtu)];
            match encoder.next_chunk(&mut buf).unwrap() {
                Some(len) => chunks.push(buf[..len].to_vec()),
                None => return chunks,
            }
        }
    }

    #[test]
    fn data_length_follows_the_design_formula() {
        // Default MTU 23 gives 17 bytes of data per chunk, as docs/ble-design.md states.
        assert_eq!(chunk_data_len(DEFAULT_MTU), Some(17));
        assert_eq!(chunk_data_len(247), Some(241));
        assert_eq!(chunk_data_len(6), None);
    }

    #[test]
    fn the_header_is_sequence_then_flags() {
        let rec = record(&[1, 2, 3]);
        let chunks = encode_all(&[&rec], 247);
        assert_eq!(chunks.len(), 1);
        assert_eq!(&chunks[0][..2], &0u16.to_le_bytes());
        assert_eq!(
            chunks[0][2],
            FLAG_END_OF_SEGMENT | FLAG_END_OF_TRANSFER,
            "a single short segment ends both"
        );
        assert_eq!(&chunks[0][3..], &rec[..]);
    }

    #[test]
    fn long_segments_split_and_only_the_last_chunk_is_flagged() {
        let payload: Vec<u8> = (0..40).collect();
        let rec = record(&payload);
        let chunks = encode_all(&[&rec], 23);
        assert!(chunks.len() > 1);
        for chunk in &chunks[..chunks.len() - 1] {
            assert_eq!(chunk[2], 0, "no end flags on interior chunks");
        }
        let last = chunks.last().unwrap();
        assert_eq!(last[2], FLAG_END_OF_SEGMENT | FLAG_END_OF_TRANSFER);
        for chunk in &chunks {
            assert!(chunk.len() <= 23 - ATT_HEADER_LEN);
        }
    }

    #[test]
    fn flags_mark_segment_ends_but_only_the_final_transfer_end() {
        let a = record(&[10]);
        let b = record(&[20, 21]);
        let chunks = encode_all(&[&a, &b], 247);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0][2], FLAG_END_OF_SEGMENT);
        assert_eq!(chunks[1][2], FLAG_END_OF_SEGMENT | FLAG_END_OF_TRANSFER);
        assert_eq!(&chunks[1][..2], &1u16.to_le_bytes());
    }

    #[test]
    fn round_trip_returns_every_record_byte_for_byte() {
        let records = [
            record(&[1, 2, 3]),
            record(&(0..30).collect::<Vec<u8>>()),
            record(&[]),
            record(&[0xff; 12]),
        ];
        let refs: Vec<&[u8]> = records.iter().map(Vec::as_slice).collect();
        let chunks = encode_all(&refs, 23);

        let mut reassembler: Reassembler<64> = Reassembler::new();
        let mut decoded = Vec::new();
        for chunk in &chunks {
            if let Some(rec) = reassembler.push(chunk).unwrap() {
                decoded.push(rec.to_vec());
            }
        }
        assert!(reassembler.finished());
        assert_eq!(decoded, records.to_vec());
    }

    #[test]
    fn a_corrupt_payload_fails_the_crc() {
        let mut rec = record(&[1, 2, 3]);
        let last = rec.len() - 1;
        rec[last] ^= 0x40;
        let chunks = encode_all(&[&rec], 247);
        let mut reassembler: Reassembler<64> = Reassembler::new();
        assert_eq!(reassembler.push(&chunks[0]), Err(TransferError::BadCrc));
    }

    #[test]
    fn a_wrong_magic_byte_is_refused() {
        let mut rec = record(&[1]);
        rec[0] = 0x00;
        let chunks = encode_all(&[&rec], 247);
        let mut reassembler: Reassembler<64> = Reassembler::new();
        assert_eq!(reassembler.push(&chunks[0]), Err(TransferError::BadMagic));
    }

    #[test]
    fn a_lost_chunk_is_reported_as_out_of_order() {
        let rec = record(&(0..40).collect::<Vec<u8>>());
        let chunks = encode_all(&[&rec], 23);
        let mut reassembler: Reassembler<64> = Reassembler::new();
        reassembler.push(&chunks[0]).unwrap();
        assert_eq!(
            reassembler.push(&chunks[2]),
            Err(TransferError::OutOfOrder {
                expected: 1,
                got: 2
            })
        );
    }

    #[test]
    fn chunks_after_the_end_are_refused() {
        let rec = record(&[7]);
        let chunks = encode_all(&[&rec], 247);
        let mut reassembler: Reassembler<64> = Reassembler::new();
        reassembler.push(&chunks[0]).unwrap();
        assert_eq!(reassembler.push(&chunks[0]), Err(TransferError::AfterEnd));
    }

    #[test]
    fn a_segment_larger_than_the_buffer_is_refused() {
        let rec = record(&[0u8; 40]);
        let chunks = encode_all(&[&rec], 247);
        let mut reassembler: Reassembler<16> = Reassembler::new();
        assert_eq!(
            reassembler.push(&chunks[0]),
            Err(TransferError::RecordTooLarge)
        );
    }

    #[test]
    fn a_small_output_buffer_is_reported_and_retryable() {
        let rec = record(&[1, 2, 3, 4]);
        let refs = [rec.as_slice()];
        let mut encoder = ChunkEncoder::new(&refs, 247).unwrap();
        let mut small = [0u8; 4];
        assert_eq!(
            encoder.next_chunk(&mut small),
            Err(TransferError::BufferTooSmall)
        );
        let mut big = [0u8; 64];
        let len = encoder.next_chunk(&mut big).unwrap().unwrap();
        assert_eq!(&big[3..len], &rec[..]);
    }

    #[test]
    fn an_mtu_without_room_for_data_is_refused() {
        let refs: [&[u8]; 0] = [];
        assert_eq!(
            ChunkEncoder::new(&refs, 6).unwrap_err(),
            TransferError::MtuTooSmall
        );
    }
}

#[cfg(test)]
mod interop_tests {
    use super::*;

    /// Two segment records, built from the spec with an independent CRC-32 and
    /// decoded by `SegmentAssembler` in `pwa/lib/ble.ts`, which accepts them.
    const RECORD_1: &str = "a51c0051c7b03c93cf0000018bcfe56800cf0000018bcfe65260cb40934a0000000000";
    const RECORD_2: &str = "a51c00f27fb8c093cf0000018bcfe6eea0cf0000018bcfe765c4cb406f480000000000";

    /// The six chunks for those records at MTU 23, as the PWA receives them.
    const CHUNKS: [&str; 6] = [
        "000000a51c0051c7b03c93cf0000018bcfe56800",
        "010000cf0000018bcfe65260cb40934a00000000",
        "02000100",
        "030000a51c00f27fb8c093cf0000018bcfe6eea0",
        "040000cf0000018bcfe765c4cb406f4800000000",
        "05000300",
    ];

    fn from_hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn encoder_output_matches_the_chunks_the_pwa_accepts() {
        let record_1 = from_hex(RECORD_1);
        let record_2 = from_hex(RECORD_2);
        let records = [record_1.as_slice(), record_2.as_slice()];
        let mut encoder = ChunkEncoder::new(&records, 23).unwrap();

        let mut produced = Vec::new();
        let mut buf = [0u8; 23];
        while let Some(len) = encoder.next_chunk(&mut buf).unwrap() {
            produced.push(to_hex(&buf[..len]));
        }
        assert_eq!(produced, CHUNKS);
    }

    #[test]
    fn the_pwa_vectors_decode_back_to_the_same_records() {
        let mut reassembler: Reassembler<64> = Reassembler::new();
        let mut decoded = Vec::new();
        for chunk in CHUNKS {
            if let Some(record) = reassembler.push(&from_hex(chunk)).unwrap() {
                decoded.push(to_hex(record));
            }
        }
        assert!(reassembler.finished());
        assert_eq!(decoded, [RECORD_1, RECORD_2]);
    }
}
