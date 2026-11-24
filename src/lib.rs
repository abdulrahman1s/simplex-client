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
pub mod net;
pub mod ratchet;
pub mod smp;
pub mod store;
pub mod tls;
pub mod types;
pub mod xftp;
