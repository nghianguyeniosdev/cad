use std::time::Duration;

/// How many times to retry a `Transient` failure, and how long to wait between
/// attempts. A `Fatal` failure is never retried; `AuthExpired` is handled by
/// the SessionCoordinator, not here.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Number of retries after the initial attempt (so up to `max_retries + 1`
    /// total attempts).
    pub max_retries: usize,
    /// Delay between attempts.
    pub backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff: Duration::from_millis(200),
        }
    }
}
