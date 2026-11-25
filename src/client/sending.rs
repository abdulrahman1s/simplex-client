//! A send's handle: the tag its request goes with, and what becomes of it told against it.

use super::error::Result;
use super::handle::Client;
use crate::types::Request;

/// Names one send. What becomes of it arrives as [`Event::SendStatus`](super::Event::SendStatus)
/// with this handle: queued (with the message it made) or refused, then sent or failed for
/// the friend or for each member of a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SendHandle(pub u64);

impl Client {
    /// Sends `request` tagged, so its outcome is told against the handle.
    pub(super) fn tagged(&self, request: Request) -> Result<SendHandle> {
        let tag = self.next_tag();
        self.request(Request::Tagged {
            tag,
            request: Box::new(request),
        })?;
        Ok(SendHandle(tag))
    }
}
