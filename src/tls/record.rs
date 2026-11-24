//! The TLS 1.3 record layer: framing, and ChaCha20-Poly1305 protection per direction (RFC 8446 section 5).
use chacha20poly1305::ChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
#[cfg(feature = "native")]
use std::io::Read;

use super::error::TlsError;
use super::key_schedule::{Secret, expand_label};

pub(super) const CCS: u8 = 20;
pub(super) const ALERT: u8 = 21;
pub(super) const HANDSHAKE: u8 = 22;
pub(super) const APPLICATION_DATA: u8 = 23;

pub(super) const MAX_PLAINTEXT: usize = 16384;
const MAX_CIPHERTEXT: usize = MAX_PLAINTEXT + 256;
const TAG_LEN: usize = 16;

/// Key, IV and sequence number of one direction.
pub(super) struct Protection {
    aead: ChaCha20Poly1305,
    iv: [u8; 12],
    seq: u64,
}

impl Protection {
    pub(super) fn new(secret: &Secret) -> Self {
        let key: [u8; 32] = expand_label(secret, "key", &[]);
        Self {
            aead: ChaCha20Poly1305::new(&key.into()),
            iv: expand_label(secret, "iv", &[]),
            seq: 0,
        }
    }

    /// A wrapped sequence number would reuse a nonce, so the connection ends first (RFC 8446 section 5.3).
    fn next_nonce(&mut self) -> Result<[u8; 12], TlsError> {
        let mut nonce = self.iv;
        for (n, s) in nonce[4..].iter_mut().zip(self.seq.to_be_bytes()) {
            *n ^= s;
        }
        self.seq = (self.seq.checked_add(1)).ok_or(TlsError::Internal("record sequence number"))?;
        Ok(nonce)
    }

    #[cfg(all(test, feature = "native"))]
    pub(super) fn at_seq(mut self, seq: u64) -> Self {
        self.seq = seq;
        self
    }

    /// A whole record (header and ciphertext) carrying `content` of type `ty`.
    pub(super) fn seal(&mut self, ty: u8, content: &[u8]) -> Result<Vec<u8>, TlsError> {
        let mut inner = content.to_vec();
        inner.push(ty);
        let len = inner.len() + TAG_LEN;
        let header = [APPLICATION_DATA, 3, 3, (len >> 8) as u8, len as u8];
        let nonce = self.next_nonce()?;
        let sealed = self
            .aead
            .encrypt(
                &nonce.into(),
                Payload {
                    msg: &inner,
                    aad: &header,
                },
            )
            .map_err(|_| TlsError::Internal("record seal"))?;
        Ok([&header[..], &sealed].concat())
    }

    /// The inner content type and content of a protected record.
    pub(super) fn open(&mut self, header: &[u8], body: &[u8]) -> Result<(u8, Vec<u8>), TlsError> {
        let nonce = self.next_nonce()?;
        let mut inner = self
            .aead
            .decrypt(
                &nonce.into(),
                Payload {
                    msg: body,
                    aad: header,
                },
            )
            .map_err(|_| TlsError::BadRecordMac)?;
        // TLSInnerPlaintext, padding included, is at most 2^14 + 1 bytes (RFC 8446 section 5.4).
        if inner.len() > MAX_PLAINTEXT + 1 {
            return Err(TlsError::RecordOverflow);
        }
        while inner.last() == Some(&0) {
            inner.pop();
        }
        let ty = inner
            .pop()
            .ok_or(TlsError::Decode("record without content type"))?;
        Ok((ty, inner))
    }
}

pub(super) struct RawRecord {
    pub(super) header: [u8; 5],
    pub(super) body: Vec<u8>,
}

/// Bytes read ahead of the next record. A read timeout leaves them intact, so it can be retried.
#[derive(Default)]
pub(super) struct Inbound {
    buf: Vec<u8>,
}

impl Inbound {
    /// Appends bytes from the network; the caller then takes every complete record, which leaves
    /// at most one partial record (5 + 2^14 + 256 bytes) behind.
    pub(super) fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    #[cfg(feature = "native")]
    pub(super) fn next_record(&mut self, sock: &mut impl Read) -> Result<RawRecord, TlsError> {
        loop {
            if let Some(record) = self.take_record()? {
                return Ok(record);
            }
            let mut chunk = [0u8; 8192];
            let n = sock.read(&mut chunk)?;
            if n == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into());
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    pub(super) fn take_record(&mut self) -> Result<Option<RawRecord>, TlsError> {
        let Some(&[ty, major, _, hi, lo]) = self.buf.first_chunk::<5>() else {
            return Ok(None);
        };
        let len = usize::from(u16::from_be_bytes([hi, lo]));
        if major != 3 {
            return Err(TlsError::Illegal("record version"));
        }
        if len > MAX_CIPHERTEXT || (ty != APPLICATION_DATA && len > MAX_PLAINTEXT) {
            return Err(TlsError::RecordOverflow);
        }
        if self.buf.len() < 5 + len {
            return Ok(None);
        }
        let body = self.buf[5..5 + len].to_vec();
        let header = [ty, major, self.buf[2], hi, lo];
        self.buf.drain(..5 + len);
        Ok(Some(RawRecord { header, body }))
    }
}
