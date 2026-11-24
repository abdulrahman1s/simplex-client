//! One dialled pipe and the session riding it: the tunnel hello first, then TLS bytes both ways.
use crate::smp::address::{DEFAULT_PORT, SmpServer};
use crate::smp::config::ConnConfig;
use crate::smp::error::SmpError;
use crate::smp::session::SmpSession;

use super::carrier::Carrier;
use super::pipe::{Pipe, PipeId};

pub(super) struct Link {
    pub(super) carrier: Carrier,
    pub(super) session: SmpSession,
    /// When it was dialled.
    pub(super) started: u64,
}

/// Where a pipe goes: the relay's first host that is not an onion one, and its port.
pub(super) fn target(server: &SmpServer) -> Result<(&str, u16), SmpError> {
    let host = (server.hosts.iter())
        .find(|h| !h.ends_with(".onion"))
        .ok_or(SmpError::Io(
            "the relay has no host reachable without Tor".into(),
        ))?;
    Ok((host, server.port.unwrap_or(DEFAULT_PORT)))
}

impl Link {
    /// The session starts now, so its connect timeout also bounds a tunnel that never answers.
    pub(super) fn new(
        id: PipeId,
        pipe: Box<dyn Pipe>,
        tunnel: bool,
        server: &SmpServer,
        cfg: &ConnConfig,
        now: u64,
    ) -> Result<Self, SmpError> {
        let (session, hello) = SmpSession::start(server, cfg, now)?;
        Ok(Self {
            carrier: Carrier::new(id, pipe, tunnel, hello),
            session,
            started: now,
        })
    }

    pub(super) fn on_open(
        &mut self,
        server: &SmpServer,
        token: Option<&str>,
    ) -> Result<(), SmpError> {
        let (host, port) = target(server)?;
        self.carrier.on_open(host, port, token)
    }

    /// Everything the session has for the socket, in frames the tunnel accepts.
    pub(super) fn flush(&mut self) -> Result<(), SmpError> {
        if !self.carrier.is_through() {
            return Ok(());
        }
        while let Some(bytes) = self.session.take_outgoing() {
            self.carrier.send(&bytes)?;
        }
        Ok(())
    }
}
