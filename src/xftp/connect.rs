//! Dialling an XFTP server: TCP, TLS 1.3 with ALPN `xftp/1`, HTTP/2, then the handshake, each step against a deadline (Client.hs `getXFTPClient`).
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

use crate::h2::{Connection, Options};
use crate::net::{Deadline, NetError, Watchdog, connect_tcp};
use crate::smp::SystemClock;
use crate::tls::{TlsStream, verify_server_chain};

use super::address::XftpServer;
use super::address::authority;
use super::client::{ClientConfig, XftpClient};
use super::error::{HandshakeError, XftpError};
use super::handshake::{Negotiated, Patience, TlsFacts, client_handshake};
use super::version::ALPN;

pub struct TcpXftpClient {
    pub client: XftpClient<TlsStream<TcpStream>>,
    socket: TcpStream,
}

fn connect_error(e: impl std::fmt::Display) -> XftpError {
    XftpError::Connect(e.to_string())
}

/// Tries each host of the address that is not a Tor host; the first that completes the handshake wins.
pub fn connect(server: &XftpServer, cfg: &ClientConfig) -> Result<TcpXftpClient, XftpError> {
    let mut last = XftpError::Connect("the address has no host that can be dialled".into());
    for host in server.dialable_hosts() {
        match connect_host(server, host, cfg) {
            Ok(c) => return Ok(c),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn connect_host(
    server: &XftpServer,
    host: &str,
    cfg: &ClientConfig,
) -> Result<TcpXftpClient, XftpError> {
    let port = server.port_or_default();
    let clock = SystemClock::new();
    let deadline = Deadline::new(&clock, cfg.connect_timeout);
    let sock = match connect_tcp(host, port, &deadline) {
        Ok(sock) => sock,
        Err(NetError::TimedOut { .. }) => return Err(XftpError::TimedOut),
        Err(e) => return Err(connect_error(e)),
    };
    let control = sock.try_clone()?;
    let left = deadline.left().ok_or(XftpError::TimedOut)?;
    // TLS gets what is left of the connect timeout, the two handshake requests one each, as upstream.
    let tls = within(left, &control, || {
        TlsStream::connect(sock, &[ALPN]).map_err(connect_error)
    })?;
    control.set_read_timeout(Some(cfg.poll))?;
    control.set_write_timeout(Some(cfg.write_timeout))?;
    let authority = authority(host, port);
    let (conn, session) = within(cfg.connect_timeout * 2, &control, || {
        handshake(tls, server, &authority, cfg, &clock)
    })?;
    Ok(TcpXftpClient {
        client: XftpClient {
            conn,
            authority,
            session,
            cfg: *cfg,
        },
        socket: control,
    })
}

/// Runs `step`, shutting `sock` if it takes longer than `budget`, which a dripping peer could
/// otherwise stretch without end.
fn within<T>(
    budget: Duration,
    sock: &TcpStream,
    step: impl FnOnce() -> Result<T, XftpError>,
) -> Result<T, XftpError> {
    let cut = sock.try_clone()?;
    let dog = Watchdog::arm(budget, move || {
        let _ = cut.shutdown(Shutdown::Both); // already closed is the goal
    })?;
    let done = step();
    if dog.disarm() {
        return Err(XftpError::TimedOut);
    }
    done
}

/// HTTP/2, then the XFTP handshake. A server that does not speak the handshake is taken at
/// version 1, as Haskell's client does.
fn handshake(
    tls: TlsStream<TcpStream>,
    server: &XftpServer,
    authority: &str,
    cfg: &ClientConfig,
    clock: &SystemClock,
) -> Result<(Connection<TlsStream<TcpStream>>, Negotiated), XftpError> {
    let tls_unique = *tls.tls_unique();
    let (chain, alpn) = (
        tls.peer_cert_chain().to_vec(),
        tls.info().alpn().map(str::to_owned),
    );
    let mut conn = Connection::open(tls, Options::default())?;
    let facts = TlsFacts {
        tls_unique: &tls_unique,
        peer_chain: &chain,
    };
    if alpn.as_deref() != Some(ALPN) {
        verify_server_chain(&chain, &server.key_hash).map_err(|_| HandshakeError::Identity)?;
        let session_id = tls_unique.to_vec();
        return Ok((
            conn,
            Negotiated {
                version: 1,
                session_id,
            },
        ));
    }
    let patience = Patience {
        clock,
        timeout: cfg.connect_timeout,
    };
    let n = client_handshake(&mut conn, authority, &server.key_hash, &facts, &patience)?;
    Ok((conn, n))
}

impl TcpXftpClient {
    /// GOAWAY, then the socket is shut down; the peer may be gone already.
    pub fn close(&mut self) {
        self.client.conn.close();
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}
