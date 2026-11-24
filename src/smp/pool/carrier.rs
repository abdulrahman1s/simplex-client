//! A dialled pipe as a byte stream: the tunnel hello first, the TLS ClientHello held until the tunnel says yes, then frames the tunnel accepts (the Worker's tunnel protocol v1).
use crate::smp::error::SmpError;
use crate::smp::tunnel::{MAX_FRAME, tunnel_hello};

use super::pipe::{Pipe, PipeId};

/// Shared by the relay links and the file links; the session riding it is the owner's.
pub struct Carrier {
    pub id: PipeId,
    pub pipe: Box<dyn Pipe>,
    /// The TLS ClientHello, held until the tunnel answers `{"ok":true}`.
    hello: Option<Vec<u8>>,
    /// The pipe is open and, through a tunnel, the tunnel hello went out.
    pub opened: bool,
    /// Through the tunnel, not direct TCP.
    tunnel: bool,
}

impl Carrier {
    #[must_use]
    pub fn new(id: PipeId, pipe: Box<dyn Pipe>, tunnel: bool, client_hello: Vec<u8>) -> Self {
        Self {
            id,
            pipe,
            hello: Some(client_hello),
            opened: false,
            tunnel,
        }
    }

    /// Bytes flow once the tunnel said yes and our ClientHello went out.
    #[must_use]
    pub fn is_through(&self) -> bool {
        self.opened && self.hello.is_none()
    }

    /// The pipe opened: through a tunnel the hello names `host:port`, straight to the server
    /// TLS begins at once.
    pub fn on_open(&mut self, host: &str, port: u16, token: Option<&str>) -> Result<(), SmpError> {
        self.opened = true;
        if !self.tunnel {
            return self.send_client_hello();
        }
        self.pipe.send_text(&tunnel_hello(host, port, token))
    }

    /// Sends our ClientHello after the tunnel's `{"ok":true}`.
    pub fn send_client_hello(&mut self) -> Result<(), SmpError> {
        match self.hello.take() {
            Some(hello) => self.send(&hello),
            None => Ok(()),
        }
    }

    /// `bytes` in frames the tunnel accepts.
    pub fn send(&mut self, bytes: &[u8]) -> Result<(), SmpError> {
        bytes
            .chunks(MAX_FRAME)
            .try_for_each(|frame| self.pipe.send_binary(frame))
    }
}
