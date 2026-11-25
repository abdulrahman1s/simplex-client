//! What a client starts with: where its store lives, the key that seals it, its name and relays.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::routing::{ProxyFallback, ProxyMode};

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
    /// When messages go through one of our relays first; `ProxyMode::default()` is the chat's.
    pub proxy_mode: ProxyMode,
    /// When a message may go directly after all; `ProxyFallback::default()` is the chat's.
    pub proxy_fallback: ProxyFallback,
    /// The content types that arrive as `Event::Data` instead of a message.
    pub data_tags: BTreeSet<String>,
}
