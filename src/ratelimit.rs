use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config::RateLimitConfig;

/// Per-client-IP token-bucket rate limiter.
///
/// Each source IP gets a bucket that refills at `requests_per_minute` and holds
/// at most `burst_size` tokens. A connection is allowed only if a token is
/// available. This bounds authentication brute-forcing and connection floods
/// from a single source.
pub struct RateLimiter {
    refill_per_sec: f64,
    burst: f64,
    buckets: Mutex<HashMap<IpAddr, Bucket>>,
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(config: &RateLimitConfig) -> Self {
        let burst = config.burst_size.max(1) as f64;
        Self {
            refill_per_sec: config.requests_per_minute as f64 / 60.0,
            burst,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if a request from `ip` is allowed, consuming one token.
    pub fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut guard = self.buckets.lock().unwrap();

        // Opportunistically evict stale buckets so the map can't grow without
        // bound under IP-spoofed floods.
        if guard.len() > 10_000 {
            guard.retain(|_, b| now.duration_since(b.last_refill) < Duration::from_secs(600));
        }

        let bucket = guard.entry(ip).or_insert_with(|| Bucket {
            tokens: self.burst,
            last_refill: now,
        });

        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.refill_per_sec).min(self.burst);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_then_throttle() {
        let limiter = RateLimiter::new(&RateLimitConfig {
            requests_per_minute: 60,
            burst_size: 3,
        });
        let ip: IpAddr = "203.0.113.7".parse().unwrap();

        // Burst of 3 is allowed immediately.
        assert!(limiter.check(ip));
        assert!(limiter.check(ip));
        assert!(limiter.check(ip));
        // 4th within the same instant is denied (refill is ~1/sec here).
        assert!(!limiter.check(ip));

        // A different source has its own independent bucket.
        let other: IpAddr = "203.0.113.8".parse().unwrap();
        assert!(limiter.check(other));
    }
}
