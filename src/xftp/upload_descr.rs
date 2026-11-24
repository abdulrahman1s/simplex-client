//! Descriptions of an uploaded file: the sender's, to delete it, and one per recipient (Agent.hs `sndFileToDescrs`).
use crate::crypto::{CbNonce, SbKey};

use super::address::XftpServer;
use super::chunks::ChunkSpec;
use super::description::{ChunkReplica, FileChunk, FileDescription, Party};

/// The encrypted file as its descriptions name it.
pub(super) struct Sealed {
    pub(super) key: SbKey,
    pub(super) nonce: CbNonce,
    /// SHA-512 of the encrypted file.
    pub(super) digest: Vec<u8>,
    pub(super) enc_size: u64,
}

/// A chunk one server holds, with the ids and keys of its sender and recipients.
pub(super) struct SentChunk {
    pub(super) spec: ChunkSpec,
    pub(super) digest: Vec<u8>,
    pub(super) server: XftpServer,
    pub(super) sender_id: Vec<u8>,
    pub(super) sender_key: Vec<u8>,
    /// Id and PKCS#8 key per recipient.
    pub(super) recipients: Vec<(Vec<u8>, Vec<u8>)>,
    pub(super) expires: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uploaded {
    /// Keeps the replica keys that delete the file; keep it to call `delete_file` later.
    pub sender: FileDescription,
    /// One description per recipient.
    pub recipients: Vec<FileDescription>,
    pub encrypted_size: u64,
    /// The earliest time (epoch seconds) a server may drop a chunk, when the servers say.
    pub expires: Option<i64>,
}

/// The descriptions and the earliest expiry of `sent`, made for `recipients`.
pub(super) fn uploaded(sealed: &Sealed, sent: &[SentChunk], recipients: usize) -> Option<Uploaded> {
    let expires = (sent.iter().map(|c| c.expires))
        .collect::<Option<Vec<_>>>()
        .and_then(|v| v.into_iter().min());
    let (sender, recipients) = describe(sealed, sent, recipients)?;
    Some(Uploaded {
        sender,
        recipients,
        encrypted_size: sealed.enc_size,
        expires,
    })
}

fn chunks(
    sent: &[SentChunk],
    replica: impl Fn(&SentChunk) -> Option<ChunkReplica>,
) -> Option<Vec<FileChunk>> {
    sent.iter()
        .zip(1u32..)
        .map(|(c, chunk_no)| {
            Some(FileChunk {
                chunk_no,
                chunk_size: c.spec.size,
                digest: c.digest.clone(),
                replicas: vec![replica(c)?],
            })
        })
        .collect()
}

fn description(
    party: Party,
    prepared: &Sealed,
    sent: &[SentChunk],
    chunks: Vec<FileChunk>,
) -> FileDescription {
    FileDescription {
        party,
        size: prepared.enc_size,
        digest: prepared.digest.clone(),
        key: prepared.key.as_bytes().to_vec(),
        nonce: prepared.nonce.as_bytes().to_vec(),
        chunk_size: sent.first().map_or(0, |c| c.spec.size),
        chunks,
        redirect: None,
    }
}

/// The sender description and one recipient description per recipient key of each chunk.
fn describe(
    prepared: &Sealed,
    sent: &[SentChunk],
    recipients: usize,
) -> Option<(FileDescription, Vec<FileDescription>)> {
    let sender_chunks = chunks(sent, |c| {
        Some(ChunkReplica {
            server: c.server.without_auth(),
            replica_id: c.sender_id.clone(),
            replica_key: c.sender_key.clone(),
        })
    })?;
    let sender = description(Party::Sender, prepared, sent, sender_chunks);
    let rcvs = (0..recipients)
        .map(|n| {
            let cs = chunks(sent, |c| {
                let (id, key) = c.recipients.get(n)?;
                Some(ChunkReplica {
                    server: c.server.without_auth(),
                    replica_id: id.clone(),
                    replica_key: key.clone(),
                })
            })?;
            Some(description(Party::Recipient, prepared, sent, cs))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((sender, rcvs))
}

#[cfg(test)]
mod tests {
    use crate::xftp::description::encode_description;

    use super::*;

    #[test]
    fn a_servers_password_never_reaches_a_description() {
        let server = format!(
            "xftp://{}@files.example:5443",
            "da1aH3nOT-9G8lV7bWamhxpDYdJ1xmW7j3JpGaDR5Ug=:s3cret"
        )
        .parse()
        .unwrap();
        let prepared = Sealed {
            key: SbKey::new([1; 32]),
            nonce: CbNonce::new([2; 24]),
            digest: vec![3; 64],
            enc_size: 65536,
        };
        let sent = [SentChunk {
            spec: ChunkSpec {
                offset: 0,
                size: 65536,
            },
            digest: vec![4; 32],
            server,
            sender_id: b"snd".to_vec(),
            sender_key: vec![5; 48],
            recipients: vec![(b"rcv".to_vec(), vec![6; 48])],
            expires: None,
        }];
        let (sender, recipients) = describe(&prepared, &sent, 1).unwrap();
        for fd in recipients.iter().chain([&sender]) {
            let text = encode_description(fd);
            assert!(!text.contains("s3cret"), "{text}");
            assert!(text.contains("@files.example:5443"), "{text}");
        }
    }
}
