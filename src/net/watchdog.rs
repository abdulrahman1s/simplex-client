//! Ends a stalled blocking connect by shutting its socket when time is up; a per-read timeout cannot, as a peer that drips bytes resets it.
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{RecvTimeoutError, Sender, bounded};

pub struct Watchdog {
    stop: Sender<()>,
    fired: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl Watchdog {
    /// Runs `cut` once `budget` has passed, unless `disarm` came first. Dropped without
    /// `disarm`, the watch also ends: its channel closes.
    pub fn arm(budget: Duration, cut: impl FnOnce() + Send + 'static) -> io::Result<Self> {
        let (stop, wait) = bounded::<()>(0);
        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        let thread = thread::Builder::new()
            .name("connect-deadline".into())
            .spawn(move || {
                if wait.recv_timeout(budget) == Err(RecvTimeoutError::Timeout) {
                    flag.store(true, Ordering::Release);
                    cut();
                }
            })?;
        Ok(Self {
            stop,
            fired,
            thread,
        })
    }

    /// Ends the watch. True when the time ran out first: the socket was shut, whatever the
    /// connect returned.
    #[must_use]
    pub fn disarm(self) -> bool {
        drop(self.stop);
        let _ = self.thread.join(); // the thread only waits and shuts a socket
        self.fired.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_cuts_once_its_budget_has_passed() {
        let cut = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&cut);
        let dog = Watchdog::arm(Duration::from_millis(20), move || {
            seen.store(true, Ordering::Release);
        })
        .unwrap();
        thread::sleep(Duration::from_millis(200));
        assert!(dog.disarm());
        assert!(cut.load(Ordering::Acquire));
    }

    #[test]
    fn watchdog_disarmed_in_time_never_cuts() {
        let cut = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&cut);
        let dog = Watchdog::arm(Duration::from_secs(30), move || {
            seen.store(true, Ordering::Release);
        })
        .unwrap();
        assert!(!dog.disarm());
        assert!(!cut.load(Ordering::Acquire));
    }
}
