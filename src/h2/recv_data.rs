//! Response body bytes and their flow control (RFC 9113 sections 6.1, 6.9).
use super::client::H2Client;
use super::error::H2Error;
use super::frames::{END_STREAM, FrameHeader, encode_window_update};
use super::recv::unpadded;
use super::state::Exchange;

impl H2Client {
    pub(super) fn on_data(
        &mut self,
        ex: Option<&mut Exchange>,
        h: FrameHeader,
        p: &[u8],
    ) -> Result<(), H2Error> {
        if h.stream == 0 {
            return Err(H2Error::Protocol("DATA on stream 0"));
        }
        let data = unpadded(h.flags, p)?;
        self.conn_recv_consumed = self.conn_recv_consumed.saturating_add(p.len() as u32);
        if self.conn_recv_consumed >= self.opts.connection_window / 2 {
            self.out
                .extend(encode_window_update(0, self.conn_recv_consumed));
            self.conn_recv_consumed = 0;
        }
        if let Some(ex) = ex.filter(|e| e.id == h.stream) {
            ex.recv_window -= p.len() as i64;
            if ex.recv_window < 0 {
                return Err(H2Error::FlowControl("server exceeded the stream window"));
            }
            if ex.body.len() + data.len() > ex.max_body {
                return Err(H2Error::TooLarge(ex.max_body));
            }
            ex.body.extend_from_slice(data);
            let full = i64::from(self.opts.stream_window);
            if h.flags & END_STREAM == 0 && ex.recv_window < full / 2 {
                (self.out).extend(encode_window_update(ex.id, (full - ex.recv_window) as u32));
                ex.recv_window = full;
            }
            ex.ended |= h.flags & END_STREAM != 0;
        }
        Ok(())
    }
}
