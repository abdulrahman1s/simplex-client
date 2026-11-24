//! TLS under one session, cut into transport blocks (Transport.hs `tPutBlock`, `tGetBlock` over `getTLS`).
use std::collections::VecDeque;

use crate::smp::error::{HandshakeError, SmpError};
use crate::smp::version::BLOCK_SIZE;
use crate::tls::{Pin, TlsClient, TlsError};

fn tls_err(e: TlsError) -> SmpError {
    match e {
        TlsError::Identity(_) => HandshakeError::Identity.into(),
        e => SmpError::Io(e.to_string()),
    }
}

pub(super) struct Link {
    tls: TlsClient,
    /// Application bytes short of a whole block: under `BLOCK_SIZE` once `feed` returns.
    partial: Vec<u8>,
    /// Bytes for the socket in the order they were sealed; the owner drains it after every call.
    out: VecDeque<Vec<u8>>,
}

impl Link {
    pub(super) fn start(pin: Pin) -> Result<Self, SmpError> {
        let (tls, hello) = TlsClient::start_pinned(&["smp/1"], pin).map_err(tls_err)?;
        Ok(Self {
            tls,
            partial: Vec::new(),
            out: VecDeque::from([hello]),
        })
    }

    pub(super) fn tls(&self) -> &TlsClient {
        &self.tls
    }

    /// Takes network bytes and returns the whole blocks they complete.
    pub(super) fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, SmpError> {
        self.tls.feed(bytes).map_err(tls_err)?;
        // Records TLS made while reading (Finished, a KeyUpdate answer) were sealed before any data written later.
        while let Some(record) = self.tls.take_outgoing() {
            self.out.push_back(record);
        }
        self.partial.extend(self.tls.read_app());
        let whole = self.partial.len() - self.partial.len() % BLOCK_SIZE;
        Ok(self
            .partial
            .drain(..whole)
            .collect::<Vec<u8>>()
            .chunks(BLOCK_SIZE)
            .map(<[u8]>::to_vec)
            .collect())
    }

    pub(super) fn write(&mut self, block: &[u8]) -> Result<(), SmpError> {
        let records = self.tls.write_app(block).map_err(tls_err)?;
        self.out.push_back(records);
        Ok(())
    }

    pub(super) fn take_outgoing(&mut self) -> Option<Vec<u8>> {
        self.out.pop_front()
    }

    pub(super) fn is_peer_closed(&self) -> bool {
        self.tls.is_peer_closed()
    }

    /// Queues close_notify when the handshake has finished.
    pub(super) fn close(&mut self) -> Result<(), SmpError> {
        let records = self.tls.close().map_err(tls_err)?;
        if !records.is_empty() {
            self.out.push_back(records);
        }
        Ok(())
    }
}
