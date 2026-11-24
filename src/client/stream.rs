//! Reading what happens: the blocking iterator and the one-at-a-time wait.

use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::RecvTimeoutError;

use super::error::{Error, Result};
use super::event::Event;
use super::handle::Client;

impl Client {
    /// Everything that happens, in order, blocking until the next event; ends when the client
    /// closes. Clones share one queue: each event goes to one reader. Answers to waiting
    /// methods are not repeated here. Up to 1024 unread events are kept, then the oldest go.
    pub fn events(&self) -> impl Iterator<Item = Event> + Send + 'static {
        // Weak, so an iterator left running does not keep a dropped client's engine alive.
        let weak = Arc::downgrade(&self.inner);
        self.inner.events.clone().into_iter().map_while(move |q| {
            let client = Client {
                inner: weak.upgrade()?,
            };
            Some(Event::new(&client, q))
        })
    }

    /// The next event, waiting at most `timeout`.
    pub fn next_event(&self, timeout: Duration) -> Result<Event> {
        let q = self
            .inner
            .events
            .recv_timeout(timeout)
            .map_err(|e| match e {
                RecvTimeoutError::Timeout => Error::Timeout,
                RecvTimeoutError::Disconnected => Error::Closed,
            })?;
        Ok(Event::new(self, q))
    }
}
