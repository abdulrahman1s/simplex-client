//! Someone asking to connect through the user's address: accept or reject.

use crate::client::{Client, Result};
use crate::types::model as wire;

#[derive(Clone, Debug)]
pub struct FriendRequest {
    client: Client,
    info: wire::FriendRequest,
}

impl FriendRequest {
    pub(in crate::client) const fn new(client: Client, info: wire::FriendRequest) -> Self {
        Self { client, info }
    }

    #[must_use]
    pub const fn id(&self) -> i64 {
        self.info.id
    }

    /// The name they gave.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// They become a friend; `Event::FriendAdd` follows once connected.
    pub fn accept(&self) -> Result<()> {
        self.client.accept(self.id())
    }

    pub fn reject(&self) -> Result<()> {
        self.client.reject(self.id())
    }
}
