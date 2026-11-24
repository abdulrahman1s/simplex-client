//! Test seams: hand JS an engine event, and see the requests the objects send.
use super::client::Client;
use crate::types::{Event, Request};

impl Client {
    /// Hands JS `event` as if the engine had said it: a test seam.
    #[doc(hidden)]
    pub fn inject(&self, event: Event) {
        if let Some(core) = self.shared.core.borrow_mut().as_mut() {
            core.engine.emit(event);
        }
        self.shared.run();
    }

    /// The requests sent since the last look, once `tap(true)` turned watching on: a test seam.
    #[doc(hidden)]
    pub fn tap(&self, on: bool) -> Vec<Request> {
        let mut tap = self.shared.tap.borrow_mut();
        let seen = tap.take().unwrap_or_default();
        *tap = on.then(Vec::new);
        seen
    }
}
