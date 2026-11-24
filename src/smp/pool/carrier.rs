//! A dialled pipe as a byte stream: the tunnel hello and the TLS ClientHello together, nothing back until the tunnel says yes, then frames the tunnel accepts (the Worker's tunnel protocol v1).
use crate::smp::error::SmpError;
use crate::smp::tunnel::{MAX_FRAME, tunnel_hello};

use super::pipe::{Pipe, PipeId};

/// Shared by the relay links and the file links; the session riding it is the owner's.
pub struct Carrier {
    pub id: PipeId,
    pub pipe: Box<dyn Pipe>,
    /// The TLS ClientHello, sent as soon as the pipe opens.
    hello: Option<Vec<u8>>,
    /// The pipe is open and, through a tunnel, the tunnel hello went out.
    pub opened: bool,
    /// The tunnel answered `{"ok":true}`; straight to the server there is nothing to wait for.
    accepted: bool,
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
            accepted: !tunnel,
            tunnel,
        }
    }

    /// Bytes flow both ways once the tunnel said yes. Until then the only bytes out are the
    /// ClientHello, and any bytes in are not from the relay.
    #[must_use]
    pub fn is_through(&self) -> bool {
        self.opened && self.accepted
    }

    /// The pipe opened: through a tunnel the hello names `host:port`, and the ClientHello
    /// follows at once rather than a round trip later. The Worker holds it until the relay's
    /// socket opens, and drops it with the tunnel if the relay is refused.
    pub fn on_open(&mut self, host: &str, port: u16, token: Option<&str>) -> Result<(), SmpError> {
        self.opened = true;
        if self.tunnel {
            self.pipe.send_text(&tunnel_hello(host, port, token))?;
        }
        match self.hello.take() {
            Some(hello) => self.send(&hello),
            None => Ok(()),
        }
    }

    /// The tunnel's `{"ok":true}`: the relay's bytes may follow.
    pub fn on_accepted(&mut self) {
        self.accepted = true;
    }

    /// `bytes` in frames the tunnel accepts.
    pub fn send(&mut self, bytes: &[u8]) -> Result<(), SmpError> {
        bytes
            .chunks(MAX_FRAME)
            .try_for_each(|frame| self.pipe.send_binary(frame))
    }
}
