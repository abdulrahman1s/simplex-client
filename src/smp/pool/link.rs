//! One dialled pipe and the session riding it: the tunnel hello first, then TLS bytes both ways.
use crate::smp::address::{DEFAULT_PORT, SmpServer};
use crate::smp::config::ConnConfig;
use crate::smp::error::SmpError;
use crate::smp::session::SmpSession;
use crate::smp::tunnel::{MAX_FRAME, tunnel_hello};

use super::pipe::{Pipe, PipeId};

pub(super) struct Link {
    pub(super) id: PipeId,
    pub(super) pipe: Box<dyn Pipe>,
    pub(super) session: SmpSession,
    /// The TLS ClientHello, held until the tunnel answers `{"ok":true}`.
    pub(super) hello: Option<Vec<u8>>,
    /// The pipe is open and, through a tunnel, the tunnel hello went out.
    pub(super) opened: bool,
    /// Through the tunnel, not direct TCP.
    pub(super) tunnel: bool,
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
            id,
            pipe,
            session,
            hello: Some(hello),
            opened: false,
            tunnel,
        })
    }

    /// Bytes flow once the tunnel said yes and our ClientHello went out.
    pub(super) fn is_through(&self) -> bool {
        self.opened && self.hello.is_none()
    }

    /// The pipe opened: through a tunnel the hello names the relay, straight to the relay
    /// TLS begins at once.
    pub(super) fn on_open(
        &mut self,
        server: &SmpServer,
        token: Option<&str>,
    ) -> Result<(), SmpError> {
        self.opened = true;
        if !self.tunnel {
            return self.send_client_hello();
        }
        let (host, port) = target(server)?;
        self.pipe.send_text(&tunnel_hello(host, port, token))
    }

    /// Sends our ClientHello after the tunnel's `{"ok":true}`.
    pub(super) fn send_client_hello(&mut self) -> Result<(), SmpError> {
        match self.hello.take() {
            Some(hello) => self.send(&hello),
            None => Ok(()),
        }
    }

    /// Everything the session has for the socket, in frames the tunnel accepts.
    pub(super) fn flush(&mut self) -> Result<(), SmpError> {
        if !self.is_through() {
            return Ok(());
        }
        while let Some(bytes) = self.session.take_outgoing() {
            self.send(&bytes)?;
        }
        Ok(())
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), SmpError> {
        bytes
            .chunks(MAX_FRAME)
            .try_for_each(|frame| self.pipe.send_binary(frame))
    }
}
