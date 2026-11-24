//! Cutting pushed network bytes into whole frames; a partial frame waits for the rest.
use super::error::H2Error;
use super::frames::{FrameHeader, HEADER_LEN, parse_header};

#[derive(Default)]
pub(super) struct FrameReader {
    /// Bytes pushed; `buf[pos..]` are not taken yet. Eviction: `push` drops the taken part, so
    /// it holds at most one push and under one frame (`HEADER_LEN` + the maximum payload).
    buf: Vec<u8>,
    pos: usize,
}

impl FrameReader {
    /// Drops the taken bytes once per push, so many small frames cost linear time, not quadratic.
    pub(super) fn push(&mut self, bytes: &[u8]) {
        self.buf.drain(..self.pos);
        self.pos = 0;
        self.buf.extend_from_slice(bytes);
    }

    /// The next whole frame, or `None` until more bytes come.
    pub(super) fn take(
        &mut self,
        max_payload: usize,
    ) -> Result<Option<(FrameHeader, Vec<u8>)>, H2Error> {
        let rest = &self.buf[self.pos..];
        let Some(head) = rest.first_chunk::<HEADER_LEN>() else {
            return Ok(None);
        };
        let header = parse_header(head);
        if header.length > max_payload {
            return Err(H2Error::FrameSize(
                "frame larger than the announced maximum",
            ));
        }
        let end = HEADER_LEN + header.length;
        let Some(payload) = rest.get(HEADER_LEN..end) else {
            return Ok(None);
        };
        let payload = payload.to_vec();
        self.pos += end;
        Ok(Some((header, payload)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taking_a_frame_leaves_the_bytes_behind_it_in_place_until_the_next_push() {
        let mut r = FrameReader::default();
        let empty_frame = [0u8, 0, 0, 0xfa, 0, 0, 0, 0, 0];
        r.push(&empty_frame.repeat(3));
        let start = r.buf.as_ptr();
        for _ in 0..3 {
            let (h, payload) = r.take(16_384).unwrap().unwrap();
            assert_eq!((h.length, payload.len()), (0, 0));
            assert_eq!(r.buf.len(), 27, "no bytes moved per frame");
        }
        assert_eq!(r.buf.as_ptr(), start);
        assert!(r.take(16_384).unwrap().is_none());
        r.push(&empty_frame[..4]);
        assert_eq!(r.buf.len(), 4, "the push dropped the taken frames");
        assert!(r.take(16_384).unwrap().is_none());
        r.push(&empty_frame[4..]);
        assert!(r.take(16_384).unwrap().is_some());
    }
}
