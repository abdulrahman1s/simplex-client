//! What became of a message sent with `Request::Tagged`, told against its tag.

use serde::{Deserialize, Serialize};

use crate::types::kind::ErrorCode;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c", rename_all = "snake_case")]
pub enum SendStatus {
    /// Taken and queued to go. `item` is the message it made here, if it made one
    /// (application data makes none).
    Queued { item: Option<i64> },
    /// Refused before anything went, for the reason `Event::Failed` would give.
    Refused { code: ErrorCode, detail: String },
    /// The relay took it: for the friend, or for one member of a group.
    Sent,
    /// It could not be delivered: for the friend, or for one member of a group.
    Failed { detail: String },
}
