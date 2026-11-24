//! One deadline over a whole blocking connect, read through a `Clock` (Client.hs `runClient`, `timeout tcpConnectTimeout`).
use std::time::Duration;

use crate::smp::Clock;

pub struct Deadline<'a> {
    clock: &'a dyn Clock,
    end_ms: u64,
}

impl<'a> Deadline<'a> {
    #[must_use]
    pub fn new(clock: &'a dyn Clock, budget: Duration) -> Self {
        let budget = u64::try_from(budget.as_millis()).unwrap_or(u64::MAX);
        Self {
            clock,
            end_ms: clock.now_ms().saturating_add(budget),
        }
    }

    /// The time still left, or `None` once the deadline has passed.
    #[must_use]
    pub fn left(&self) -> Option<Duration> {
        let left = self.end_ms.saturating_sub(self.clock.now_ms());
        (left > 0).then(|| Duration::from_millis(left))
    }
}
