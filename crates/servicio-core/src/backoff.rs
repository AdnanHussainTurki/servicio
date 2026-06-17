use std::time::Duration;

/// Exponential backoff + crash-loop detection. Pure, deterministic.
#[derive(Debug, Clone, Copy)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    max_retries: u32,
    reset_window: Duration,
}

impl Backoff {
    pub fn new(base: Duration, max: Duration, max_retries: u32, reset_window: Duration) -> Self {
        Self { base, max, max_retries, reset_window }
    }

    /// Delay before retry `attempt` (1-based): base * 2^(attempt-1), capped at `max`.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        let factor = 2u64.saturating_pow(attempt - 1);
        let secs = self.base.as_secs().saturating_mul(factor);
        Duration::from_secs(secs).min(self.max)
    }

    /// True when the retry count has exceeded `max_retries`.
    pub fn is_crash_loop(&self, retries: u32) -> bool {
        retries > self.max_retries
    }

    /// True when an instance stayed up long enough to reset its retry counter.
    pub fn should_reset(&self, uptime: Duration) -> bool {
        uptime > self.reset_window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b() -> Backoff {
        Backoff::new(Duration::from_secs(1), Duration::from_secs(60), 5, Duration::from_secs(30))
    }

    #[test]
    fn first_delay_is_base() {
        assert_eq!(b().delay_for_attempt(1), Duration::from_secs(1));
    }

    #[test]
    fn delay_doubles_each_attempt() {
        let bo = b();
        assert_eq!(bo.delay_for_attempt(2), Duration::from_secs(2));
        assert_eq!(bo.delay_for_attempt(3), Duration::from_secs(4));
        assert_eq!(bo.delay_for_attempt(4), Duration::from_secs(8));
    }

    #[test]
    fn delay_is_capped_at_max() {
        // attempt 10 would be 512s uncapped; cap is 60s.
        assert_eq!(b().delay_for_attempt(10), Duration::from_secs(60));
    }

    #[test]
    fn crash_loop_trips_after_max_retries() {
        assert!(!b().is_crash_loop(5)); // exactly at limit is still allowed
        assert!(b().is_crash_loop(6));  // one past the limit trips
    }

    #[test]
    fn uptime_beyond_reset_window_resets_counter() {
        let bo = b();
        assert!(bo.should_reset(Duration::from_secs(31)));
        assert!(!bo.should_reset(Duration::from_secs(29)));
    }
}
