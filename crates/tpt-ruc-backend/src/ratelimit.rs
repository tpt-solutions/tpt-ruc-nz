// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Fixed-window rate limiting, keyed by client IP address.
//!
//! Two limits apply. The first caps the number of requests a client may make in
//! a minute. The second blocks a client for a while after repeated failed API
//! key checks, which slows down guessing. A blocked client is refused even with
//! the right key until the block ends.
//!
//! The limiter sees the TCP peer address. Behind a reverse proxy, every client
//! shares the proxy's address, so the limits then apply to all clients together.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Requests a client may make in one window.
pub const REQUESTS_PER_WINDOW: u32 = 120;
/// Length of the request window.
pub const REQUEST_WINDOW: Duration = Duration::from_secs(60);
/// Failed API key checks allowed in one window before the client is blocked.
pub const AUTH_FAILURES_PER_WINDOW: u32 = 10;
/// Length of the failure window, and how long a client stays blocked.
pub const AUTH_WINDOW: Duration = Duration::from_secs(15 * 60);
/// Most clients tracked at once. Old entries are dropped when this is reached.
const MAX_TRACKED: usize = 10_000;

/// Both limiters, shared by every connection.
pub struct RateLimiter {
    requests: FixedWindow,
    auth_failures: FixedWindow,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            requests: FixedWindow::new(REQUESTS_PER_WINDOW, REQUEST_WINDOW),
            auth_failures: FixedWindow::new(AUTH_FAILURES_PER_WINDOW, AUTH_WINDOW),
        }
    }
}

impl RateLimiter {
    /// Counts one request from `client`. Returns how long to wait when the request must be refused.
    pub fn check_request(&self, client: IpAddr, now: Instant) -> Result<(), Duration> {
        if let Some(wait) = self.auth_failures.full(client, now) {
            return Err(wait);
        }
        self.requests.hit(client, now)
    }

    /// Records one failed API key check from `client`.
    pub fn record_auth_failure(&self, client: IpAddr, now: Instant) {
        // The return value only says whether the client is still under the limit.
        let _ = self.auth_failures.hit(client, now);
    }
}

/// Counts hits per client in windows of fixed length.
struct FixedWindow {
    limit: u32,
    window: Duration,
    counts: Mutex<HashMap<IpAddr, Count>>,
}

#[derive(Clone, Copy)]
struct Count {
    started: Instant,
    hits: u32,
}

impl FixedWindow {
    fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            counts: Mutex::new(HashMap::new()),
        }
    }

    /// Records one hit. Refused with the wait time once the limit is reached.
    fn hit(&self, client: IpAddr, now: Instant) -> Result<(), Duration> {
        let mut counts = self.lock();
        self.forget_expired(&mut counts, now);
        let count = counts.entry(client).or_insert(Count {
            started: now,
            hits: 0,
        });
        if now.duration_since(count.started) >= self.window {
            *count = Count {
                started: now,
                hits: 0,
            };
        }
        if count.hits >= self.limit {
            return Err(self.remaining(count, now));
        }
        count.hits += 1;
        Ok(())
    }

    /// The wait time when `client` has used the whole limit in the current window.
    fn full(&self, client: IpAddr, now: Instant) -> Option<Duration> {
        let counts = self.lock();
        let count = counts.get(&client)?;
        if now.duration_since(count.started) < self.window && count.hits >= self.limit {
            Some(self.remaining(count, now))
        } else {
            None
        }
    }

    fn remaining(&self, count: &Count, now: Instant) -> Duration {
        self.window
            .saturating_sub(now.duration_since(count.started))
            .max(Duration::from_secs(1))
    }

    /// Drops expired windows once the table is large, so memory stays bounded.
    fn forget_expired(&self, counts: &mut HashMap<IpAddr, Count>, now: Instant) {
        if counts.len() >= MAX_TRACKED {
            counts.retain(|_, count| now.duration_since(count.started) < self.window);
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<IpAddr, Count>> {
        self.counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(last: u8) -> IpAddr {
        IpAddr::from([192, 0, 2, last])
    }

    #[test]
    fn requests_beyond_the_limit_are_refused_until_the_window_ends() {
        let limiter = RateLimiter::default();
        let start = Instant::now();
        for _ in 0..REQUESTS_PER_WINDOW {
            assert!(limiter.check_request(client(1), start).is_ok());
        }
        let wait = limiter.check_request(client(1), start).unwrap_err();
        assert!(wait <= REQUEST_WINDOW && wait >= Duration::from_secs(1));

        let later = start + REQUEST_WINDOW;
        assert!(limiter.check_request(client(1), later).is_ok());
    }

    #[test]
    fn clients_are_counted_separately() {
        let limiter = RateLimiter::default();
        let start = Instant::now();
        for _ in 0..REQUESTS_PER_WINDOW {
            limiter.check_request(client(1), start).unwrap();
        }
        assert!(limiter.check_request(client(2), start).is_ok());
    }

    #[test]
    fn repeated_auth_failures_block_even_a_correct_request() {
        let limiter = RateLimiter::default();
        let start = Instant::now();
        for _ in 0..AUTH_FAILURES_PER_WINDOW {
            limiter.record_auth_failure(client(3), start);
        }
        assert!(limiter.check_request(client(3), start).is_err());
        assert!(limiter.check_request(client(4), start).is_ok());

        let after_block = start + AUTH_WINDOW;
        assert!(limiter.check_request(client(3), after_block).is_ok());
    }

    #[test]
    fn expired_windows_do_not_keep_growing_the_table() {
        let window = FixedWindow::new(1, Duration::from_secs(1));
        let start = Instant::now();
        for last in 0..=255u8 {
            for offset in 0..50u8 {
                let ip = IpAddr::from([10, 0, offset, last]);
                let _ = window.hit(ip, start);
            }
        }
        let later = start + Duration::from_secs(2);
        let _ = window.hit(client(9), later);
        assert!(window.lock().len() <= MAX_TRACKED);
    }
}
