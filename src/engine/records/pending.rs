//! Messages waiting for a group member whose connection is not up yet (Store/Messages.hs `pending_group_messages`).

use serde::{Deserialize, Serialize};

/// The reference keeps a message 30 days (Commands.hs `cleanupMessages`), and its pending
/// rows go with it.
pub const PENDING_TTL_MS: i64 = 30 * 24 * 3600 * 1000;

/// No more wait for one member; past this the oldest goes.
pub const MAX_PENDING: usize = 1000;

/// One message, kept oldest first in the member's queue (`keys::pending`). Eviction: sent and
/// removed on the member's CON, dropped with its connection, after `PENDING_TTL_MS`, and the
/// oldest past `MAX_PENDING`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingMsg {
    #[serde(with = "crate::types::model::bytes")]
    pub body: Vec<u8>,
    /// Milliseconds since the epoch, when it was queued.
    pub at: i64,
    /// The message's shared id: the user's item with it gets its delivery state once sent.
    #[serde(with = "crate::types::model::bytes::opt", default)]
    pub shared: Option<Vec<u8>>,
}
