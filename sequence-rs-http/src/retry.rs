//! Client-side throttling and retry policy. These live in the HTTP crate so
//! `ReqwestClient` can apply them without depending on the root crate.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The server's per-key rate limit: 100 requests/minute.
pub const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 100;

/// Client-side rate limiting via a token bucket sized to `requests_per_minute`:
/// bursts up to a minute's allowance, then holds that rate. `None` on `Config`
/// disables it.
#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    pub requests_per_minute: u32,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            requests_per_minute: DEFAULT_RATE_LIMIT_PER_MINUTE,
        }
    }
}

/// Controls how transient failures (`429`, `502/503/504`, `500` on idempotent
/// requests, and network timeouts/connect errors) are retried.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Total attempts including the first (so `3` = 1 try + 2 retries).
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub multiplier: f64,
    /// On `429`, wait `max(Retry-After, backoff)` when true.
    pub respect_retry_after: bool,
    /// Apply full jitter to the backoff to avoid synchronized retries.
    pub jitter: bool,
    /// Optional hard ceiling on total time across the whole operation. On expiry
    /// the in-flight request is cancelled and a timeout error returned. `None`
    /// bounds retries by `max_attempts` alone.
    pub max_elapsed_time: Option<Duration>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
            respect_retry_after: true,
            jitter: true,
            max_elapsed_time: None,
        }
    }
}

impl RetryPolicy {
    /// A policy that never retries (1 attempt only).
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            ..Self::default()
        }
    }

    /// Backoff before the next attempt (`attempt` is 1-based). A `429`'s
    /// `retry_after` (seconds) wins when `respect_retry_after` is set.
    pub(crate) fn backoff(&self, attempt: u32, retry_after: Option<u64>) -> Duration {
        let exp = self.initial_backoff.as_millis() as f64
            * self.multiplier.powi(attempt.saturating_sub(1) as i32);
        let capped = (exp as u64).min(self.max_backoff.as_millis() as u64);
        let jittered = if self.jitter {
            (capped as f64 * rand::random::<f64>()) as u64
        } else {
            capped
        };
        let mut delay = Duration::from_millis(jittered);
        if let (true, Some(secs)) = (self.respect_retry_after, retry_after) {
            delay = delay.max(Duration::from_secs(secs));
        }
        delay
    }
}

#[derive(Debug)]
struct TokenState {
    /// May go negative to reserve a slot for a queued request; the deficit is
    /// what that request waits out.
    tokens: f64,
    last: Instant,
}

/// Token bucket: capacity `requests_per_minute`, refilling at that rate per
/// minute. Shared across clones via `Arc`.
#[derive(Debug)]
pub(crate) struct Throttle {
    capacity: f64,
    refill_per_sec: f64,
    state: Mutex<TokenState>,
}

impl Throttle {
    pub(crate) fn new(requests_per_minute: u32) -> Self {
        let rpm = requests_per_minute.max(1) as f64;
        Self {
            capacity: rpm,
            refill_per_sec: rpm / 60.0,
            state: Mutex::new(TokenState {
                tokens: rpm, // start full: allow an immediate burst
                last: Instant::now(),
            }),
        }
    }

    /// Reserve a token as of `now`, returning the wait before firing (`None` =
    /// now). Sync so the token math is testable with a controlled clock.
    fn reserve(&self, now: Instant) -> Option<Duration> {
        let mut s = self.state.lock().unwrap();
        let elapsed = now.saturating_duration_since(s.last).as_secs_f64();
        s.tokens = (s.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        s.last = now;
        s.tokens -= 1.0;
        (s.tokens < 0.0).then(|| Duration::from_secs_f64(-s.tokens / self.refill_per_sec))
    }

    /// Wait until this request is permitted to fire, reserving its slot.
    pub(crate) async fn acquire(&self) {
        if let Some(wait) = self.reserve(Instant::now()) {
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_jitter() -> RetryPolicy {
        RetryPolicy {
            jitter: false,
            ..RetryPolicy::default()
        }
    }

    #[test]
    fn backoff_grows_exponentially_and_caps() {
        let p = no_jitter();
        assert_eq!(p.backoff(1, None), Duration::from_millis(500));
        assert_eq!(p.backoff(2, None), Duration::from_millis(1000));
        assert_eq!(p.backoff(3, None), Duration::from_millis(2000));
        // Far-out attempts are clamped to max_backoff (30s).
        assert_eq!(p.backoff(20, None), Duration::from_secs(30));
    }

    #[test]
    fn retry_after_takes_precedence_when_larger() {
        let p = no_jitter();
        // Retry-After of 5s beats the 500ms computed backoff.
        assert_eq!(p.backoff(1, Some(5)), Duration::from_secs(5));
        // ...but a tiny Retry-After doesn't shrink a larger computed backoff.
        assert_eq!(p.backoff(3, Some(1)), Duration::from_millis(2000));
    }

    #[test]
    fn retry_after_ignored_when_disabled() {
        let p = RetryPolicy {
            jitter: false,
            respect_retry_after: false,
            ..RetryPolicy::default()
        };
        assert_eq!(p.backoff(1, Some(99)), Duration::from_millis(500));
    }

    #[test]
    fn jitter_stays_within_cap() {
        let p = RetryPolicy::default();
        for _ in 0..1000 {
            assert!(p.backoff(3, None) <= Duration::from_millis(2000));
        }
    }

    #[test]
    fn throttle_allows_burst_up_to_capacity() {
        let t = Throttle::new(60); // capacity 60, refill 1 token/sec
        let now = Instant::now();
        // A full bucket lets 60 requests through at the same instant.
        for _ in 0..60 {
            assert!(t.reserve(now).is_none());
        }
        // The 61st must wait for a token to refill (~1s).
        let wait = t.reserve(now).expect("bucket is empty, must throttle");
        assert!(wait > Duration::ZERO && wait <= Duration::from_secs(2));
    }

    #[test]
    fn throttle_refills_over_time() {
        let t = Throttle::new(60); // 1 token/sec
        let start = Instant::now();
        for _ in 0..60 {
            let _ = t.reserve(start); // drain the bucket
        }
        // ~2s later, ~2 tokens have refilled: two immediate, the third waits.
        let later = start + Duration::from_secs(2);
        assert!(t.reserve(later).is_none());
        assert!(t.reserve(later).is_none());
        assert!(t.reserve(later).is_some());
    }
}
