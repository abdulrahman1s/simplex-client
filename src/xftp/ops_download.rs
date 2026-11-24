//! Downloading one chunk: FGET, then the server's transport encryption off the stream (Client.hs `downloadXFTPChunk`).
use std::io::{Read, Write};

use crate::crypto::{Alg, KeyPair, PrivateKey};

use super::chunk_open::{fetched_len, open_fetched};
use super::client::{Ctl, XftpClient};
use super::commands::Command;
use super::error::XftpError;

impl<T: Read + Write> XftpClient<T> {
    /// Fetches a chunk: agrees a key with the server, decrypts the stream, checks its tag and its SHA-256.
    pub fn download_chunk(
        &mut self,
        recipient_key: &PrivateKey,
        file_id: &[u8],
        chunk_size: u32,
        digest: &[u8],
        ctl: &mut Ctl<'_>,
    ) -> Result<Vec<u8>, XftpError> {
        let ours = KeyPair::generate(Alg::X25519)?;
        let get = Command::Get {
            rcv_dh_key: ours.public.clone(),
        };
        let max = fetched_len(chunk_size)?;
        let (response, body) = self.call(Some(recipient_key), file_id, &get, &[], max, ctl)?;
        open_fetched(&ours.private, response, body, chunk_size, digest)
    }
}
