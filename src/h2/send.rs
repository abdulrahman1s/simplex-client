//! Writing a request: HEADERS with CONTINUATION, then DATA within the peer's flow-control windows.
use super::client::H2Client;
use super::frames::{CONTINUATION, DATA, END_HEADERS, END_STREAM, HEADERS, append_frame};
use super::hpack_encode::encode_headers;
use super::response::Request;
use super::state::Exchange;

const BATCH: usize = 64 * 1024;

impl H2Client {
    pub(super) fn send_headers(&mut self, ex: &Exchange, req: &Request<'_>) {
        let length = req.body.len().to_string();
        let mut fields = vec![
            (":method", req.method),
            (":scheme", "https"),
            (":path", req.path),
            (":authority", req.authority),
        ];
        fields.extend_from_slice(req.headers);
        if !req.body.is_empty() {
            fields.push(("content-length", &length));
        }
        let block = encode_headers(&fields);
        let end_stream = if req.body.is_empty() { END_STREAM } else { 0 };
        let mut parts = block.chunks(self.peer_max_frame).peekable();
        let mut first = true;
        while let Some(part) = parts.next() {
            let end_headers = if parts.peek().is_none() {
                END_HEADERS
            } else {
                0
            };
            if first {
                append_frame(
                    &mut self.out,
                    HEADERS,
                    end_headers | end_stream,
                    ex.id,
                    part,
                );
            } else {
                append_frame(&mut self.out, CONTINUATION, end_headers, ex.id, part);
            }
            first = false;
        }
    }

    /// Queues up to one batch of the body that the windows, and `send_ahead`, allow.
    pub(super) fn send_batch(&mut self, ex: &mut Exchange) {
        let start = self.out.len();
        let ahead = i64::from(self.opts.send_ahead);
        while ex.sent < ex.upload.len() && self.out.len() - start < BATCH {
            let unacked = (self.peer_initial_window - ex.send_window).max(0);
            let room = (ex.send_window.min(self.conn_send))
                .min(ahead - unacked)
                .min(self.peer_max_frame as i64);
            let n = usize::try_from(room)
                .unwrap_or(0)
                .min(ex.upload.len() - ex.sent);
            if n == 0 {
                break;
            }
            let last = ex.sent + n == ex.upload.len();
            append_frame(
                &mut self.out,
                DATA,
                if last { END_STREAM } else { 0 },
                ex.id,
                &ex.upload[ex.sent..ex.sent + n],
            );
            ex.sent += n;
            ex.send_window -= n as i64;
            self.conn_send -= n as i64;
        }
    }
}
