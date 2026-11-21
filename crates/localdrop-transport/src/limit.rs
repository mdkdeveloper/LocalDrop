//! Rate limiting of failed handshakes per source IP (docs/security.md).
//!
//! Each failure costs an attacker one online password guess. After
//! [`MAX_FAILURES`] failures within [`WINDOW`], connections from that address are
//! refused before TLS until the window has passed.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Failures allowed per address within [`WINDOW`].
pub const MAX_FAILURES: u32 = 5;
/// Window for counting failures, and how long an address stays blocked.
pub const WINDOW: Duration = Duration::from_secs(60);
/// Addresses tracked at most (oldest entries are dropped first).
const MAX_TRACKED: usize = 4096;

#[derive(Debug, Clone, Copy)]
struct Entry {
    first: Instant,
    failures: u32,
}

/// Counts failed handshakes per IP address.
#[derive(Debug, Default)]
pub struct RateLimiter {
    entries: HashMap<IpAddr, Entry>,
}

impl RateLimiter {
    /// Creates an empty limiter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a new connection from `ip` may start a handshake.
    pub fn allowed(&mut self, ip: IpAddr, now: Instant) -> bool {
        match self.entries.get(&ip) {
            Some(e) if now.duration_since(e.first) >= WINDOW => {
                self.entries.remove(&ip);
                true
            }
            Some(e) => e.failures < MAX_FAILURES,
            None => true,
        }
    }

    /// Records a failed handshake from `ip`.
    pub fn failed(&mut self, ip: IpAddr, now: Instant) {
        if self.entries.len() >= MAX_TRACKED && !self.entries.contains_key(&ip) {
            self.entries
                .retain(|_, e| now.duration_since(e.first) < WINDOW);
            if self.entries.len() >= MAX_TRACKED {
                if let Some(oldest) = self
                    .entries
                    .iter()
                    .min_by_key(|(_, e)| e.first)
                    .map(|(k, _)| *k)
                {
                    self.entries.remove(&oldest);
                }
            }
        }
        let e = self.entries.entry(ip).or_insert(Entry {
            first: now,
            failures: 0,
        });
        if now.duration_since(e.first) >= WINDOW {
            *e = Entry {
                first: now,
                failures: 0,
            };
        }
        e.failures += 1;
    }

    /// Forgets `ip` after a successful handshake.
    pub fn succeeded(&mut self, ip: IpAddr) {
        self.entries.remove(&ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_max_failures_until_the_window_passes() {
        let mut l = RateLimiter::new();
        let ip: IpAddr = "192.168.1.9".parse().unwrap();
        let other: IpAddr = "192.168.1.10".parse().unwrap();
        let t0 = Instant::now();
        for i in 0..MAX_FAILURES {
            assert!(l.allowed(ip, t0), "attempt {i}");
            l.failed(ip, t0);
        }
        assert!(!l.allowed(ip, t0 + Duration::from_secs(30)));
        assert!(l.allowed(other, t0), "other addresses are not affected");
        assert!(l.allowed(ip, t0 + WINDOW));
    }

    #[test]
    fn success_resets() {
        let mut l = RateLimiter::new();
        let ip: IpAddr = "10.0.0.2".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..MAX_FAILURES - 1 {
            l.failed(ip, t0);
        }
        l.succeeded(ip);
        l.failed(ip, t0);
        assert!(l.allowed(ip, t0));
    }
}
