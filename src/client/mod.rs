//! The high-level client: start it, read its events, act on what they carry.

mod answers;
mod builder;
mod dispatch;
mod error;
mod event;
mod groups;
mod handle;
mod messages;
mod objects;
mod people;
mod roster;
mod spawn;
mod stream;
mod waiters;

#[cfg(test)]
mod tests;

pub use builder::ClientBuilder;
pub use error::{Error, Result};
pub use event::Event;
pub use handle::Client;
pub use objects::{Author, Chat, Friend, FriendRequest, Group, GroupInvite, Member, Message};
