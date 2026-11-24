//! One chunk of an in-memory upload: FNEW on the first server that takes it, FADD for the other recipients, FPUT (Agent.hs `createChunk`, `uploadFileChunk`).
use crate::crypto::{Alg, KeyPair, PublicKey, sha256};

use super::super::address::XftpServer;
use super::super::chunks::ChunkSpec;
use super::super::commands::{Command, FileInfo};
use super::super::config::MAX_PER_REQUEST;
use super::super::error::XftpError;
use super::super::upload_descr::SentChunk;
use super::step::{Answer, Call, Tries};
use super::upload_stage::{Made, Stage, bytes};

pub(super) struct ChunkWork {
    pub(super) spec: ChunkSpec,
    pub(super) digest: Vec<u8>,
    pub(super) sender: KeyPair,
    /// One key pair per recipient the chunk is made for.
    pub(super) rcv: Vec<KeyPair>,
    pub(super) stage: Stage,
    pub(super) tries: Tries,
}

fn publics(pairs: &[KeyPair]) -> Vec<PublicKey> {
    pairs.iter().map(|p| p.public.clone()).collect()
}

fn unexpected() -> XftpError {
    XftpError::Unexpected("response to the command")
}

impl ChunkWork {
    pub(super) fn new(
        spec: ChunkSpec,
        encrypted: &[u8],
        recipients: usize,
        first: usize,
    ) -> Result<Self, XftpError> {
        let rcv = (0..recipients)
            .map(|_| KeyPair::generate(Alg::Ed25519))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            spec,
            digest: sha256(bytes(encrypted, spec)).to_vec(),
            sender: KeyPair::generate(Alg::Ed25519)?,
            rcv,
            stage: Stage::Create { first, tried: 0 },
            tries: Tries::default(),
        })
    }

    pub(super) fn is_putting(&self) -> bool {
        matches!(self.stage, Stage::Put(_))
    }

    pub(super) fn call(
        &self,
        servers: &[XftpServer],
        storage_hours: Option<u32>,
        encrypted: &[u8],
    ) -> Call {
        let call = |server: &XftpServer, entity: &[u8], cmd: Command, file: Vec<u8>| Call {
            server: server.clone(),
            key: Some(self.sender.private.clone()),
            entity: entity.to_vec(),
            cmd,
            file,
            max_file: 0,
        };
        match &self.stage {
            Stage::Create { first, tried } => {
                let server = &servers[(first + tried) % servers.len()];
                let info = FileInfo {
                    snd_key: self.sender.public.clone(),
                    size: self.spec.size,
                    digest: self.digest.clone(),
                };
                let head = &self.rcv[..self.rcv.len().min(MAX_PER_REQUEST)];
                let cmd = Command::New {
                    info,
                    rcv_keys: publics(head),
                    basic_auth: server.basic_auth.clone(),
                    storage_hours,
                };
                call(server, &[], cmd, Vec::new())
            }
            Stage::Add(m) => {
                let rest = &self.rcv[m.ids.len()..];
                let batch = &rest[..rest.len().min(MAX_PER_REQUEST)];
                let cmd = Command::AddRecipients {
                    rcv_keys: publics(batch),
                };
                call(&m.server, &m.sender_id, cmd, Vec::new())
            }
            Stage::Put(m) => {
                let chunk = bytes(encrypted, self.spec).to_vec();
                call(&m.server, &m.sender_id, Command::Put, chunk)
            }
        }
    }

    /// The chunk once it is up; a failure the tries cannot get past ends the upload.
    pub(super) fn on_answer(
        &mut self,
        answer: Answer,
        servers: &[XftpServer],
    ) -> Result<Option<SentChunk>, XftpError> {
        let response = match answer {
            Ok((r, _)) => r,
            Err(e) if self.tries.again(&e) => return Ok(None),
            Err(e) => return self.next_server(e, servers.len()).map(|()| None),
        };
        self.tries.reset();
        let stage = std::mem::replace(&mut self.stage, Stage::Create { first: 0, tried: 0 });
        self.stage = match stage {
            Stage::Create { first, tried } => {
                let head = self.rcv.len().min(MAX_PER_REQUEST);
                let (sender_id, ids, expires) = (response.into_snd_ids())
                    .filter(|(_, ids, _)| ids.len() == head)
                    .ok_or_else(unexpected)?;
                let server = servers[(first + tried) % servers.len()].clone();
                self.after(Made {
                    server,
                    sender_id,
                    ids,
                    expires,
                })
            }
            Stage::Add(mut m) => {
                let want = (self.rcv.len() - m.ids.len()).min(MAX_PER_REQUEST);
                let more = (response.into_rcv_ids())
                    .filter(|ids| ids.len() == want)
                    .ok_or_else(unexpected)?;
                m.ids.extend(more);
                self.after(m)
            }
            Stage::Put(m) if response.is_ok() => return Ok(Some(self.sent(m))),
            Stage::Put(_) => return Err(unexpected()),
        };
        Ok(None)
    }
}
