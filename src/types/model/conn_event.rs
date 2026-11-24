//! What a connection-event notice says: the phase of an address switch, the state of a ratchet re-sync and a broken message chain (CIContent/Events.hs `RcvConnEvent`, `SndConnEvent`; CIContent.hs `msgIntegrityError`).

use serde::{Deserialize, Serialize};

/// `SwitchPhase` as chat shows it. Received: "started changing address for you..." and "changed
/// address for you"; the user's own: "started changing address..." and "you changed address".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchPhase {
    Started,
    Completed,
}

/// `RatchetSyncState` (`ratchetSyncStatusToText`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    /// "connection synchronized"
    Ok,
    /// "decryption error (connection may be out of sync), synchronization allowed"
    Allowed,
    /// "decryption error (connection out of sync), synchronization required"
    Required,
    /// "connection synchronization started"
    Started,
    /// "connection synchronization agreed"
    Agreed,
}

/// `MsgErrorType` of `CIRcvIntegrityError`: how a received message did not follow the one before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c", rename_all = "snake_case")]
pub enum IntegrityError {
    /// "skipped message ID from..to": messages that never arrived.
    Skipped { from: i64, to: i64 },
    /// "unexpected message ID n"
    BadId(i64),
    /// "incorrect message hash"
    BadHash,
    /// "duplicate message ID"
    Duplicate,
}
