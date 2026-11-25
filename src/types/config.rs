//! What a client starts with: where its store lives, the key that seals it, its name and relays.

use std::collections::BTreeSet;
use std::path::PathBuf;

use zeroize::Zeroizing;

use super::routing::{ProxyFallback, ProxyMode};

/// How the engine opens its store and which relays it uses. `Debug` leaves the key out.
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    /// The folder holding the store and received files.
    pub dir: PathBuf,
    /// The passphrase that seals the store, wiped when dropped. The native client takes it out
    /// as soon as the store is open.
    pub key: Zeroizing<String>,
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
    /// Send what changed in the chat list (`Event::ChatsChanged`) instead of the whole list
    /// each time; the first list, and one `Request::Refresh` asks for, are whole.
    pub roster_deltas: bool,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("dir", &self.dir)
            .field("key", &"<redacted>")
            .field("name", &self.name)
            .field("relays", &self.relays)
            .field("proxy_mode", &self.proxy_mode)
            .field("proxy_fallback", &self.proxy_fallback)
            .field("data_tags", &self.data_tags)
            .field("roster_deltas", &self.roster_deltas)
            .finish()
    }
}
