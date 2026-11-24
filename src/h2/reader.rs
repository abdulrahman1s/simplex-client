//! Cutting pushed network bytes into whole frames; a partial frame waits for the rest.
use super::error::H2Error;
use super::frames::{FrameHeader, HEADER_LEN, parse_header};

#[derive(Default)]
pub(super) struct FrameReader {
    /// Bytes of frames not yet taken; under one frame (`HEADER_LEN` + the maximum payload) once
    /// `take` has returned `None`.
    buf: Vec<u8>,
}

impl FrameReader {
    pub(super) fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// The next whole frame, or `None` until more bytes come.
    pub(super) fn take(
        &mut self,
        max_payload: usize,
    ) -> Result<Option<(FrameHeader, Vec<u8>)>, H2Error> {
        let Some(head) = self.buf.first_chunk::<HEADER_LEN>() else {
            return Ok(None);
        };
        let header = parse_header(head);
        if header.length > max_payload {
            return Err(H2Error::FrameSize(
                "frame larger than the announced maximum",
            ));
        }
        let end = HEADER_LEN + header.length;
        if self.buf.len() < end {
            return Ok(None);
        }
        let payload = self.buf[HEADER_LEN..end].to_vec();
        self.buf.drain(..end);
        Ok(Some((header, payload)))
    }
}
