//! Where one chunk's upload stands, and what each step leaves behind.
use super::super::address::XftpServer;
use super::super::chunks::ChunkSpec;
use super::super::error::XftpError;
use super::super::upload_descr::SentChunk;
use super::upload_chunk::ChunkWork;

/// The chunk as one server made it.
pub(super) struct Made {
    pub(super) server: XftpServer,
    pub(super) sender_id: Vec<u8>,
    /// One per recipient key so far; bounded by the keys.
    pub(super) ids: Vec<Vec<u8>>,
    pub(super) expires: Option<i64>,
}

pub(super) enum Stage {
    /// FNEW on the server `tried` places after the random `first`.
    Create {
        first: usize,
        tried: usize,
    },
    Add(Made),
    Put(Made),
}

/// The bytes of `spec` in the encrypted file.
pub(super) fn bytes(encrypted: &[u8], spec: ChunkSpec) -> &[u8] {
    let start = usize::try_from(spec.offset).unwrap_or(usize::MAX);
    let end = start.saturating_add(spec.size as usize);
    encrypted.get(start..end).unwrap_or_default()
}

impl ChunkWork {
    /// A temporary failure that outlasted its tries moves FNEW to the next server; anything
    /// else ends the upload.
    pub(super) fn next_server(&mut self, e: XftpError, servers: usize) -> Result<(), XftpError> {
        match &mut self.stage {
            Stage::Create { tried, .. } if e.is_temporary() && *tried + 1 < servers => {
                *tried += 1;
                self.tries.reset();
                Ok(())
            }
            Stage::Create { .. } | Stage::Add(_) | Stage::Put(_) => Err(e),
        }
    }

    /// FADD while some recipients have no id yet, then FPUT.
    pub(super) fn after(&self, m: Made) -> Stage {
        if m.ids.len() < self.rcv.len() {
            Stage::Add(m)
        } else {
            Stage::Put(m)
        }
    }

    pub(super) fn sent(&self, m: Made) -> SentChunk {
        let recipients = (m.ids.into_iter().zip(&self.rcv))
            .map(|(id, k)| (id, k.private.to_der().to_vec()))
            .collect();
        SentChunk {
            spec: self.spec,
            digest: self.digest.clone(),
            server: m.server,
            sender_id: m.sender_id,
            sender_key: self.sender.private.to_der().to_vec(),
            recipients,
            expires: m.expires,
        }
    }
}
