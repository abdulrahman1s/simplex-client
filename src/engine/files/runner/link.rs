//! One file server over a dialled pipe: the tunnel's carrier with an XFTP session riding it.
use crate::h2;
use crate::smp::{Carrier, Dialer, PipeId, PipeInput, TunnelError, parse_tunnel_reply};
use crate::xftp::{ClientConfig, SessionEvent, XftpError, XftpServer, XftpSession};

/// Upload bytes the server has not acknowledged, at most. An upload moves about this much per
/// round trip, as the server grants a stream about 1 MiB (http2 4.2's `properWindowSize`).
/// With TLS and HTTP/2 framing it stays under the tunnel's 1 MiB write-ahead allowance, with
/// about 250 KiB to spare; `worker/test/sendahead.test.ts` keeps it there.
const SEND_AHEAD: u32 = 768 * 1024;

pub(super) struct FileLink {
    pub(super) server: XftpServer,
    pub(super) carrier: Carrier,
    pub(super) session: XftpSession,
    host: String,
    port: u16,
}

/// How a tunnel refusal reads to a transfer: a temporary failure, with a longer wait when the
/// Worker counts too many tunnels from this address.
pub(super) fn refused(e: &TunnelError) -> (XftpError, bool) {
    let limited = matches!(e, TunnelError::RateLimited | TunnelError::TooManyTunnels);
    (XftpError::Connect(format!("tunnel: {e}")), limited)
}

impl FileLink {
    pub(super) fn dial(
        dialer: &mut dyn Dialer,
        id: PipeId,
        server: &XftpServer,
        now: u64,
    ) -> Result<Self, XftpError> {
        let host = (server.dialable_hosts().next())
            .ok_or_else(|| {
                XftpError::Connect("the address has no host that can be dialled".into())
            })?
            .to_owned();
        let port = server.port_or_default();
        let opts = h2::Options {
            send_ahead: SEND_AHEAD,
            ..h2::Options::default()
        };
        let (session, hello) =
            XftpSession::start(server, &host, ClientConfig::default(), opts, now)?;
        let tunnel = dialer.tunnelled();
        let pipe = (dialer.dial(id, &host, port)).map_err(|e| XftpError::Connect(e.to_string()))?;
        Ok(Self {
            server: server.clone(),
            carrier: Carrier::new(id, pipe, tunnel, hello),
            session,
            host,
            port,
        })
    }

    /// What the pipe reported, as the session's events; a pipe that failed ends the session.
    /// The flag says the tunnel refused for too many tunnels.
    pub(super) fn on_input(
        &mut self,
        input: PipeInput,
        now: u64,
        token: Option<&str>,
    ) -> (Vec<SessionEvent>, bool) {
        let failed = match input {
            PipeInput::Open if !self.carrier.opened => {
                (self.carrier.on_open(&self.host, self.port, token))
                    .err()
                    .map(|e| (e.to_string(), false))
            }
            PipeInput::Open => None,
            PipeInput::Text(text) => match parse_tunnel_reply(&text) {
                Ok(()) => {
                    self.carrier.on_accepted();
                    None
                }
                Err(e) => {
                    let (err, limited) = refused(&e);
                    return (self.session.end(err), limited);
                }
            },
            PipeInput::Binary(_) if !self.carrier.is_through() => {
                Some(("bytes before the tunnel opened".to_owned(), false))
            }
            PipeInput::Binary(bytes) => return (self.session.feed(&bytes, now), false),
            PipeInput::Closed { code, reason } => {
                Some((format!("the pipe closed ({code} {reason})"), false))
            }
        };
        match failed {
            Some((why, limited)) => (self.session.end(XftpError::Connect(why)), limited),
            None => (Vec::new(), false),
        }
    }

    /// Sends what the session has, once the tunnel lets bytes through.
    pub(super) fn flush(&mut self) -> Vec<SessionEvent> {
        if !self.carrier.is_through() {
            return Vec::new();
        }
        while let Some(bytes) = self.session.take_outgoing() {
            if let Err(e) = self.carrier.send(&bytes) {
                return self.session.end(XftpError::Connect(e.to_string()));
            }
        }
        Vec::new()
    }

    /// GOAWAY and close_notify when they can go, then the pipe closes.
    pub(super) fn close(mut self) {
        drop(self.session.close());
        self.flush();
        self.carrier.pipe.close();
    }
}
