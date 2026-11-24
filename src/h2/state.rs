//! What a connection keeps: its options, a header block in progress, and the request in flight.
use super::client::H2Client;
use super::error::H2Error;
use super::frames::{CANCEL, MAX_WINDOW, encode_rst_stream};
use super::hpack_decode::Headers;
use super::response::{Progress, Response};

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Receive window announced per stream.
    pub stream_window: u32,
    /// Receive window announced for the whole connection.
    pub connection_window: u32,
    pub max_header_bytes: usize,
    /// Body bytes sent ahead of the peer's acknowledgement, at most, whatever window it grants;
    /// a byte pipe with a small buffer of its own (the tunnel's 1 MiB) needs this.
    pub send_ahead: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            stream_window: 8 << 20,
            connection_window: 16 << 20,
            max_header_bytes: 1 << 20,
            send_ahead: MAX_WINDOW as u32,
        }
    }
}

/// A header block waiting for its CONTINUATION frames.
pub(super) struct HeaderBlock {
    pub(super) stream: u32,
    pub(super) bytes: Vec<u8>,
    pub(super) end_stream: bool,
}

/// The state of the request in flight.
pub(super) struct Exchange {
    pub(super) id: u32,
    pub(super) send_window: i64,
    pub(super) recv_window: i64,
    pub(super) headers: Option<Headers>,
    pub(super) trailers: Headers,
    /// The response body; bounded by `max_body`.
    pub(super) body: Vec<u8>,
    pub(super) max_body: usize,
    /// The request body, sent from `sent` on.
    pub(super) upload: Vec<u8>,
    pub(super) sent: usize,
    pub(super) ended: bool,
}

impl Exchange {
    pub(super) fn new(id: u32, conn: &H2Client, upload: Vec<u8>, max_body: usize) -> Self {
        Self {
            id,
            send_window: conn.peer_initial_window,
            recv_window: i64::from(conn.opts.stream_window),
            headers: None,
            trailers: Vec::new(),
            body: Vec::new(),
            max_body,
            upload,
            sent: 0,
            ended: false,
        }
    }

    pub(super) fn progress(&self) -> Progress {
        Progress {
            sent: self.sent,
            received: self.body.len(),
        }
    }

    /// The finished response; RST_STREAM goes to `out` when the server answered before the
    /// whole body was sent.
    pub(super) fn into_response(self, out: &mut Vec<u8>) -> Result<Response, H2Error> {
        if self.sent < self.upload.len() {
            out.extend(encode_rst_stream(self.id, CANCEL));
        }
        let status = (self.headers.as_deref())
            .and_then(|h| h.iter().find(|(n, _)| n == ":status"))
            .and_then(|(_, v)| v.parse().ok());
        let (Some(status), Some(headers)) = (status, self.headers) else {
            return Err(H2Error::Protocol("response without :status"));
        };
        Ok(Response {
            status,
            headers,
            trailers: self.trailers,
            body: self.body,
        })
    }
}
