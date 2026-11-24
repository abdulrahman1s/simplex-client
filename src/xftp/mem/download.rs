//! Downloading a file into memory from its description: each chunk from its replicas in turn, then the checks and the decryption (Agent.hs receive workers, `downloadFileChunk`).
use crate::crypto::{Alg, KeyPair, PrivateKey};

use super::super::chunk_open::{fetched_len, open_fetched};
use super::super::commands::Command;
use super::super::description::FileDescription;
use super::super::error::XftpError;
use super::step::{Answer, Call, Step, Tries};

/// A downloaded file: the name its sender gave, and its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

pub struct MemDownload {
    pub(super) fd: FileDescription,
    /// `fd` is a redirect: what it holds is the real description.
    pub(super) outer: bool,
    /// The largest encrypted size taken into memory.
    pub(super) limit: u64,
    /// The encrypted chunks fetched so far, in order; bounded by `fd.size`, at most `limit`.
    pub(super) fetched: Vec<u8>,
    pub(super) chunk: usize,
    pub(super) replica: usize,
    pub(super) tries: Tries,
    /// The DH key sent with the FGET in hand.
    pub(super) dh: Option<KeyPair>,
    pub(super) last: Option<XftpError>,
}

impl MemDownload {
    /// Refuses a description that is not a recipient's, is malformed, or promises more than `limit` bytes.
    pub fn new(fd: &FileDescription, limit: u64) -> Result<Self, XftpError> {
        super::download_finish::check(fd, limit)?;
        Ok(Self {
            fd: fd.clone(),
            outer: fd.redirect.is_some(),
            limit,
            fetched: Vec::new(),
            chunk: 0,
            replica: 0,
            tries: Tries::default(),
            dh: None,
            last: None,
        })
    }

    /// Encrypted bytes fetched and the total, of the file itself: a redirect's own small file
    /// does not count.
    #[must_use]
    pub fn progress(&self) -> Option<(u64, u64)> {
        (!self.outer).then_some((self.fetched.len() as u64, self.fd.size))
    }

    /// The next FGET, or the file once every chunk is here and checked.
    pub fn next_step(&mut self) -> Result<Step<Fetched>, XftpError> {
        let Some(chunk) = self.fd.chunks.get(self.chunk) else {
            return self.finish();
        };
        let Some(r) = chunk.replicas.get(self.replica) else {
            let why = XftpError::Description(format!("chunk {} has no replicas", chunk.chunk_no));
            return Err(self.last.take().unwrap_or(why));
        };
        let key = PrivateKey::from_der(&r.replica_key)
            .map_err(|_| XftpError::Description("bad replica key".into()))?;
        let dh = KeyPair::generate(Alg::X25519)?;
        let call = Call {
            server: r.server.clone(),
            key: Some(key),
            entity: r.replica_id.clone(),
            cmd: Command::Get {
                rcv_dh_key: dh.public.clone(),
            },
            file: Vec::new(),
            max_file: fetched_len(chunk.chunk_size)?,
        };
        self.dh = Some(dh);
        Ok(Step::Call(Box::new(call)))
    }

    /// A chunk that fails its checks, or a replica that keeps failing, is tried elsewhere; a
    /// cancel ends the download.
    pub fn on_answer(&mut self, answer: Answer) -> Result<(), XftpError> {
        let (Some(chunk), Some(dh)) = (self.fd.chunks.get(self.chunk), self.dh.take()) else {
            return Err(XftpError::Unexpected("an answer with no command in hand"));
        };
        let opened = answer.and_then(|(response, body)| {
            open_fetched(&dh.private, response, body, chunk.chunk_size, &chunk.digest)
        });
        match opened {
            Ok(bytes) => {
                self.fetched.extend(bytes);
                (self.chunk, self.replica) = (self.chunk + 1, 0);
                self.tries.reset();
            }
            Err(XftpError::Cancelled) => return Err(XftpError::Cancelled),
            Err(e) if self.tries.again(&e) => {}
            Err(e) => {
                self.replica += 1;
                self.tries.reset();
                self.last = Some(e);
            }
        }
        Ok(())
    }
}
