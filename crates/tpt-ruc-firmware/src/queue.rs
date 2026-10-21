// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! A fixed-size queue of finished trip segments, for store-and-forward.
//!
//! Segments wait here until the phone takes them over BLE. When the queue is
//! full, the oldest segment is dropped and counted, so the device never stops
//! recording and the loss is visible.

use tpt_ruc_core::Segment;

use crate::flash::{SectorLog, SectorStorage};

/// Ring buffer of at most `N` segments.
#[derive(Debug, Clone, Copy)]
pub struct SegmentQueue<const N: usize> {
    items: [Option<Segment>; N],
    head: usize,
    len: usize,
    dropped: u32,
}

impl<const N: usize> Default for SegmentQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> SegmentQueue<N> {
    /// Creates an empty queue.
    pub const fn new() -> Self {
        Self {
            items: [None; N],
            head: 0,
            len: 0,
            dropped: 0,
        }
    }

    /// Adds a segment. If the queue is full, the oldest segment is dropped first.
    pub fn push(&mut self, segment: Segment) {
        if N == 0 {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        if self.len == N {
            self.head = (self.head + 1) % N;
            self.len -= 1;
            self.dropped = self.dropped.saturating_add(1);
        }
        let tail = (self.head + self.len) % N;
        self.items[tail] = Some(segment);
        self.len += 1;
    }

    /// Removes and returns the oldest segment.
    pub fn pop(&mut self) -> Option<Segment> {
        if self.len == 0 {
            return None;
        }
        let segment = self.items[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        segment
    }

    /// The oldest segment, left in the queue.
    pub fn front(&self) -> Option<Segment> {
        if self.len == 0 {
            return None;
        }
        self.items[self.head]
    }

    /// Number of segments waiting.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when no segments are waiting.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of segments dropped because the queue was full.
    pub fn dropped(&self) -> u32 {
        self.dropped
    }
}

/// Moves queued segments into the flash log, oldest first, and returns how many moved.
///
/// A segment the log refuses stays at the front of the queue, so nothing is
/// lost while flash is full or failing. Call this again once space is freed.
pub fn drain_into<const N: usize, S: SectorStorage>(
    queue: &mut SegmentQueue<N>,
    log: &mut SectorLog<S>,
) -> usize {
    let mut moved = 0;
    while let Some(segment) = queue.front() {
        if log.append(&segment).is_err() {
            break;
        }
        queue.pop();
        moved += 1;
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::tests::RamSectors;

    fn segment(start_ms: u64) -> Segment {
        Segment {
            start_ms,
            end_ms: start_ms + 1,
            distance_m: 1.0,
        }
    }

    #[test]
    fn drain_moves_segments_to_flash_and_keeps_the_rest_while_full() {
        // A 32-byte sector leaves 23 bytes after its header, enough for one
        // 19-byte record and no more, so the second segment has nowhere to go.
        let mut log = SectorLog::open(RamSectors::new(1, 32));
        let mut queue: SegmentQueue<4> = SegmentQueue::new();
        for i in 1..=3 {
            queue.push(segment(i));
        }

        assert_eq!(drain_into(&mut queue, &mut log), 1);
        assert_eq!(queue.len(), 2, "refused segments stay queued");
        assert_eq!(queue.front().map(|s| s.start_ms), Some(2));

        // Retiring the stored sector makes room, and the queue drains in order.
        assert!(log.retire_oldest().unwrap());
        assert_eq!(drain_into(&mut queue, &mut log), 1);
        assert_eq!(queue.front().map(|s| s.start_ms), Some(3));
        assert_eq!(log.oldest_sector_segments(), 1);
    }

    #[test]
    fn drain_into_an_empty_queue_moves_nothing() {
        let mut log = SectorLog::open(RamSectors::new(2, 128));
        let mut queue: SegmentQueue<2> = SegmentQueue::new();
        assert_eq!(drain_into(&mut queue, &mut log), 0);
        assert_eq!(log.oldest_sector_segments(), 0);
    }

    #[test]
    fn returns_segments_in_order() {
        let mut q: SegmentQueue<4> = SegmentQueue::new();
        q.push(segment(1));
        q.push(segment(2));
        assert_eq!(q.pop().map(|s| s.start_ms), Some(1));
        assert_eq!(q.pop().map(|s| s.start_ms), Some(2));
        assert_eq!(q.pop(), None);
        assert!(q.is_empty());
    }

    #[test]
    fn drops_the_oldest_when_full_and_counts_it() {
        let mut q: SegmentQueue<2> = SegmentQueue::new();
        q.push(segment(1));
        q.push(segment(2));
        q.push(segment(3));
        assert_eq!(q.dropped(), 1);
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop().map(|s| s.start_ms), Some(2));
        assert_eq!(q.pop().map(|s| s.start_ms), Some(3));
    }

    #[test]
    fn wraps_around_after_many_operations() {
        let mut q: SegmentQueue<3> = SegmentQueue::new();
        for i in 0..10 {
            q.push(segment(i));
            if i % 2 == 0 {
                q.pop();
            }
        }
        assert!(q.len() <= 3);
    }

    #[test]
    fn zero_capacity_drops_everything() {
        let mut q: SegmentQueue<0> = SegmentQueue::new();
        q.push(segment(1));
        assert_eq!(q.dropped(), 1);
        assert_eq!(q.pop(), None);
    }
}
