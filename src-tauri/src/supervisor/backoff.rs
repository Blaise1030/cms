use std::time::Duration;

const DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];
const STABLE_UPTIME: Duration = Duration::from_secs(30);

/// Counts consecutive failed (re)starts; a daemon that stays healthy for
/// `STABLE_UPTIME` earns a fresh budget via `reset`.
#[derive(Debug, Default)]
pub struct Backoff {
    failures: usize,
}

impl Backoff {
    /// Delay before the next restart, or `None` once the budget is spent.
    pub fn next_delay(&mut self) -> Option<Duration> {
        let delay = DELAYS.get(self.failures).copied();
        self.failures = (self.failures + 1).min(DELAYS.len());
        delay
    }

    pub fn reset(&mut self) {
        self.failures = 0;
    }

    pub fn was_stable(uptime: Duration) -> bool {
        uptime >= STABLE_UPTIME
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn delays_double_then_give_up_after_three() {
        let mut b = Backoff::default();
        assert_eq!(b.next_delay(), Some(Duration::from_secs(1)));
        assert_eq!(b.next_delay(), Some(Duration::from_secs(2)));
        assert_eq!(b.next_delay(), Some(Duration::from_secs(4)));
        assert_eq!(b.next_delay(), None);
        assert_eq!(b.next_delay(), None);
    }

    #[test]
    fn reset_restores_full_budget() {
        let mut b = Backoff::default();
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.next_delay(), Some(Duration::from_secs(1)));
    }

    #[test]
    fn stable_uptime_threshold_is_thirty_seconds() {
        assert!(!Backoff::was_stable(Duration::from_secs(29)));
        assert!(Backoff::was_stable(Duration::from_secs(30)));
    }
}
