//! Per-host rate limiting for HTTP requests.

use dashmap::DashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Per-host rate limiter to avoid hammering external servers.
pub struct RateLimiter {
    /// Minimum delay between requests to the same host.
    min_delay: Duration,
    /// Last request time per host.
    last_request: DashMap<String, Instant>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given minimum delay between requests.
    pub fn new(min_delay: Duration) -> Self {
        Self {
            min_delay,
            last_request: DashMap::new(),
        }
    }

    /// Wait if necessary before making a request to this host.
    /// This ensures we don't make requests to the same host faster than min_delay.
    pub async fn wait(&self, host: &str) {
        if self.min_delay.is_zero() {
            return;
        }

        let now = Instant::now();

        // Check if we need to wait
        if let Some(last) = self.last_request.get(host) {
            let elapsed = now.duration_since(*last);
            if elapsed < self.min_delay {
                let wait_time = self.min_delay - elapsed;
                sleep(wait_time).await;
            }
        }

        // Update last request time
        self.last_request.insert(host.to_string(), Instant::now());
    }

    /// Get the number of hosts we've tracked.
    pub fn tracked_hosts(&self) -> usize {
        self.last_request.len()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        // Default to 200ms between requests to same host
        Self::new(Duration::from_millis(200))
    }
}
