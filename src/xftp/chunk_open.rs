//! A fetched chunk: the server's transport encryption off, its tag and its SHA-256 checked (Client.hs `downloadXFTPChunk`; Lazy.hs `sbDecryptChunk`).
use subtle::ConstantTimeEq;

use crate::crypto::{CbNonce, PrivateKey, PublicKey, SbKey, SbState, dh, sha256};

use super::error::XftpError;
use super::responses::Response;
use super::version::AUTH_TAG_SIZE;

fn unexpected() -> XftpError {
    XftpError::Unexpected("response to the command")
}

/// The bytes that follow an `FGET` answer: the chunk and its tag.
pub fn fetched_len(chunk_size: u32) -> Result<usize, XftpError> {
    usize::try_from(chunk_size)
        .map(|n| n + AUTH_TAG_SIZE)
        .map_err(|_| XftpError::Unexpected("chunk size"))
}

/// The chunk from the answer to `FGET` and the bytes after it, `own` being the DH key sent with it.
pub fn open_fetched(
    own: &PrivateKey,
    answer: Response,
    mut body: Vec<u8>,
    chunk_size: u32,
    digest: &[u8],
) -> Result<Vec<u8>, XftpError> {
    let (dh_key, nonce) = answer.into_file().ok_or_else(unexpected)?;
    let size = fetched_len(chunk_size)? - AUTH_TAG_SIZE;
    if body.len() != size + AUTH_TAG_SIZE {
        return Err(XftpError::Mismatch("chunk size"));
    }
    let tag = body.split_off(size);
    open_chunk(own, &dh_key, &nonce, &mut body, &tag)?;
    if !bool::from(sha256(&body).as_slice().ct_eq(digest)) {
        return Err(XftpError::Mismatch("chunk digest"));
    }
    Ok(body)
}

/// `cbInit` and `sbDecryptChunk` over a whole chunk, then the tag (the stream is the box of the DH secret).
fn open_chunk(
    own: &PrivateKey,
    server_key: &PublicKey,
    nonce: &CbNonce,
    body: &mut [u8],
    tag: &[u8],
) -> Result<(), XftpError> {
    let secret = dh(server_key, own)?;
    if bool::from(secret.as_bytes().ct_eq(&[0u8; 32])) {
        return Err(XftpError::Mismatch("transport key"));
    }
    let mut state = SbState::new(&SbKey::from_slice(secret.as_bytes())?, nonce);
    state.decrypt(body);
    if bool::from(state.tag().ct_eq(tag)) {
        Ok(())
    } else {
        Err(XftpError::Mismatch("transport authentication tag"))
    }
}
