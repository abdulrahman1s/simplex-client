//! A minimal HTTP/2 client for XFTP, sans-I/O with a blocking driver (RFC 9113, HPACK RFC 7541), AGPL-3.0-only.

mod client;
mod connection;
mod error;
pub mod frames;
mod hpack_decode;
mod hpack_encode;
mod hpack_int;
mod huffman;
mod huffman_table;
mod inbound;
mod reader;
mod recv;
mod recv_data;
mod recv_headers;
mod response;
mod send;
mod state;
mod static_table;

pub use client::H2Client;
pub use connection::Connection;
pub use error::{H2Error, Stop};
pub use hpack_decode::{Headers, HpackDecoder};
pub use hpack_encode::encode_headers;
pub use hpack_int::{HpackError, decode_int, encode_int};
pub use huffman::{Huffman, huffman_encode};
pub use response::{Progress, Request, Response};
pub use state::Options;
