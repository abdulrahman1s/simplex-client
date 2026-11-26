//! Integration tests of the native engine's encoding and crypto modules, and the client.
#![allow(clippy::unwrap_used, reason = "tests")]

mod agent_golden;
mod agent_links;
mod agent_messages;
mod agent_short_link;
mod agent_support;
#[cfg(feature = "native")]
mod channel_live;
mod chat_batch;
mod chat_compress;
mod chat_golden_channel;
mod chat_golden_grp;
mod chat_golden_msg;
mod chat_roundtrip;
mod chat_support;
mod chat_ts_reference;
#[cfg(feature = "native")]
mod cli_ws;
#[cfg(feature = "native")]
mod client_live;
mod crypto;
mod encoding;
#[cfg(feature = "native")]
mod h2_connection;
mod h2_hpack;
mod h2_sans_io;
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
#[cfg(feature = "native")]
mod xftp_live_sans_io;
mod xftp_protocol;
