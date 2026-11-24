//! What a connection-event notice says: the phase of an address switch and the state of a ratchet re-sync (CIContent/Events.hs `RcvConnEvent`, `SndConnEvent`).

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
