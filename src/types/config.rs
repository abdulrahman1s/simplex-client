//! What a client starts with: where its store lives, the key that seals it, its name and relays.

use std::path::PathBuf;

/// How the engine opens its store and which relays it uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// The folder holding the store and received files.
    pub dir: PathBuf,
    /// The passphrase that seals the store.
    pub key: String,
    /// The display name used when the store is new.
    pub name: String,
    /// Relay addresses (`smp://…`, `xftp://…`); empty = the protocol's preset relays.
    pub relays: Vec<String>,
}
