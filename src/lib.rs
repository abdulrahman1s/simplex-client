//! A SimpleX Chat protocol client in Rust: SMP, the agent's double ratchet, chat messages,
//! groups, files and calls, ported from simplexmq and simplex-chat (AGPL-3.0-only).

#![forbid(unsafe_code)]

pub mod agent;
pub mod chat;
#[cfg(feature = "native")]
pub mod client;
pub mod crypto;
pub mod encoding;
pub mod engine;
pub mod h2;
pub mod limits;
#[cfg(feature = "native")]
pub mod net;
pub mod ratchet;
#[cfg(any(feature = "native", feature = "wasm"))]
mod roster;
pub mod smp;
pub mod store;
pub mod tls;
pub mod types;
#[cfg(feature = "wasm")]
pub mod wasm;
pub mod xftp;

#[cfg(feature = "native")]
pub use client::{
    Author, Chat, Client, ClientBuilder, Error, Event, Friend, FriendRequest, Group, GroupInvite,
    Member, Message, Result,
};
pub use types::model::*;
pub use types::{ErrorCode, ProxyMode, Request, RequestKind};
