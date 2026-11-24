//! Integration tests of the native engine's encoding and crypto modules, and the client.
#![allow(clippy::unwrap_used, reason = "tests")]

mod agent_golden;
mod agent_links;
mod agent_messages;
mod agent_short_link;
mod agent_support;
mod chat_batch;
mod chat_compress;
mod chat_golden_grp;
mod chat_golden_msg;
mod chat_ts_reference;
mod chat_roundtrip;
mod chat_support;
#[cfg(feature = "native")]
mod client_live;
mod crypto;
mod encoding;
#[cfg(feature = "native")]
mod h2_connection;
mod h2_hpack;
#[cfg(feature = "native")]
mod h2_support;
mod hex;
mod ratchet;
mod vectors;
mod xftp_description;
#[cfg(feature = "native")]
mod xftp_file;
#[cfg(feature = "native")]
mod xftp_live;
mod xftp_protocol;
