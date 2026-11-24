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
    /// The pipe is open and the tunnel hello went out.
    pub(super) opened: bool,
}

impl Link {
    /// The session starts now, so its connect timeout also bounds a tunnel that never answers.
    pub(super) fn new(
        id: PipeId,
        pipe: Box<dyn Pipe>,
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
        })
    }

    /// Bytes flow once the tunnel said yes and our ClientHello went out.
    pub(super) fn is_through(&self) -> bool {
        self.opened && self.hello.is_none()
    }

    /// The tunnel hello names the relay's first host it can reach: not an onion one.
    pub(super) fn send_tunnel_hello(
        &mut self,
        server: &SmpServer,
        token: Option<&str>,
    ) -> Result<(), SmpError> {
        let host = (server.hosts.iter())
            .find(|h| !h.ends_with(".onion"))
            .ok_or(SmpError::Io(
                "the relay has no host the tunnel can reach".into(),
            ))?;
        let port = server.port.unwrap_or(DEFAULT_PORT);
        self.opened = true;
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
