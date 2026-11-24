//! Uploading a file held in memory: encrypt it whole, then its chunks one by one, then the descriptions (Agent.hs `encryptFileForUpload`, send workers).
use crate::crypto::{CbNonce, SbKey, random_array, sha512};

use super::super::address::XftpServer;
use super::super::chunks::{
    ChunkSpec, FILE_SIZE_LEN, MAX_FILE_SIZE_HARD, prepare_chunk_sizes, prepare_chunk_specs,
};
use super::super::error::XftpError;
use super::super::file_encrypt::encrypt_bytes;
use super::super::file_header::FileHeader;
use super::super::upload_descr::{Sealed, SentChunk, Uploaded, uploaded};
use super::super::version::AUTH_TAG_SIZE;
use super::step::{Answer, Step};
use super::upload_chunk::ChunkWork;

/// Holds the encrypted file until the last chunk is up: about the file's size, plus the chunk
/// of the command in hand.
pub struct MemUpload {
    sealed: Sealed,
    encrypted: Vec<u8>,
    specs: Vec<ChunkSpec>,
    servers: Vec<XftpServer>,
    recipients: usize,
    storage_hours: Option<u32>,
    /// Bounded by `specs`.
    sent: Vec<SentChunk>,
    work: Option<ChunkWork>,
}

impl MemUpload {
    /// Encrypts `content` under a fresh key, with `name` in its header, for `recipients`.
    pub fn new(
        content: &[u8],
        name: &str,
        servers: &[XftpServer],
        recipients: usize,
        storage_hours: Option<u32>,
    ) -> Result<Self, XftpError> {
        if servers.is_empty() {
            return Err(XftpError::NoServers);
        }
        let header = FileHeader {
            file_name: name.to_owned(),
            file_extra: None,
        }
        .encode()?;
        let payload = (header.len() + content.len()) as u64;
        if payload > MAX_FILE_SIZE_HARD {
            return Err(XftpError::File(
                "the file is larger than XFTP allows".into(),
            ));
        }
        let sizes = prepare_chunk_sizes(payload + FILE_SIZE_LEN + AUTH_TAG_SIZE as u64);
        let enc_size = sizes.iter().map(|s| u64::from(*s)).sum();
        let (key, nonce) = (SbKey::random()?, CbNonce::random()?);
        let encrypted = encrypt_bytes(content, &header, &key, &nonce, enc_size)?;
        let digest = sha512(&encrypted).to_vec();
        Ok(Self {
            sealed: Sealed {
                key,
                nonce,
                digest,
                enc_size,
            },
            encrypted,
            specs: prepare_chunk_specs(&sizes),
            servers: servers.to_vec(),
            recipients: recipients.max(1),
            storage_hours,
            sent: Vec::new(),
            work: None,
        })
    }

    /// The encrypted size: what progress counts to.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.sealed.enc_size
    }

    /// Encrypted bytes on the servers so far.
    #[must_use]
    pub fn done(&self) -> u64 {
        self.sent.iter().map(|c| u64::from(c.spec.size)).sum()
    }

    /// Whether the command in hand moves a chunk, so its progress counts.
    #[must_use]
    pub fn is_putting(&self) -> bool {
        self.work.as_ref().is_some_and(ChunkWork::is_putting)
    }

    /// The next command, or the descriptions once every chunk is up.
    pub fn next_step(&mut self) -> Result<Step<Uploaded>, XftpError> {
        if self.work.is_none() {
            let Some(spec) = self.specs.get(self.sent.len()) else {
                return uploaded(&self.sealed, &self.sent, self.recipients)
                    .map(Step::Done)
                    .ok_or(XftpError::Unexpected("chunk without recipient"));
            };
            let first = usize::from(random_array::<1>()?[0]);
            self.work = Some(ChunkWork::new(
                *spec,
                &self.encrypted,
                self.recipients,
                first,
            )?);
        }
        let Some(work) = &self.work else {
            return Err(XftpError::Unexpected("no chunk in hand"));
        };
        let call = work.call(&self.servers, self.storage_hours, &self.encrypted);
        Ok(Step::Call(Box::new(call)))
    }

    /// The answer to the last command from `next`; an `Err` ends the upload.
    pub fn on_answer(&mut self, answer: Answer) -> Result<(), XftpError> {
        let Some(work) = &mut self.work else {
            return Err(XftpError::Unexpected("an answer with no command in hand"));
        };
        if let Some(chunk) = work.on_answer(answer, &self.servers)? {
            self.sent.push(chunk);
            self.work = None;
        }
        Ok(())
    }
}
