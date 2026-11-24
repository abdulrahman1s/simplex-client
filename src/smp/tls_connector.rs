//! The production connector: TCP, TLS 1.3 with ALPN `smp/1`, then the SMP handshake, all within the connect timeout (Transport/Client.hs `runTLSTransportClient`, Client.hs `runClient`).
use std::io;
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use crate::net::{Deadline, NetError, Watchdog, connect_tcp};
use crate::tls::{Pin, TlsReader, TlsStream, TlsWriter};

use super::address::SmpServer;
use super::clock::SystemClock;
use super::config::ConnConfig;
use super::error::{SmpError, tls_failure};
use super::handshake::{HandshakeConfig, TlsFacts, client_handshake};
use super::transport::{Closer, Connector, Established, Reader, Wire, Writer};

pub struct TlsConnector;

struct Rd(TlsReader<TcpStream>);
struct Wr(TlsWriter<TcpStream>);
struct SocketCloser(TcpStream);

impl Reader for Rd {
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        io::Read::read_exact(&mut self.0, buf)
    }

    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(timeout)
    }
}

impl Writer for Wr {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        io::Write::write_all(&mut self.0, buf)
    }
}

impl Closer for SocketCloser {
    fn close(&self) {
        let _ = self.0.shutdown(Shutdown::Both); // already closed is the goal
    }
}

fn io_err(e: impl std::fmt::Display) -> SmpError {
    SmpError::Io(e.to_string())
}

impl Connector for TlsConnector {
    fn connect(
        &self,
        server: &SmpServer,
        host: &str,
        cfg: &ConnConfig,
        closer_slot: &dyn Fn(Arc<dyn Closer>),
    ) -> Result<Wire, SmpError> {
        let clock = SystemClock::new();
        let deadline = Deadline::new(&clock, cfg.connect_timeout);
        let sock = match connect_tcp(host, server.port_or_default(), &deadline) {
            Ok(sock) => sock,
            Err(NetError::TimedOut { .. }) => return Err(SmpError::Timeout),
            Err(
                e @ (NetError::OnionUnsupported(_)
                | NetError::Resolve { .. }
                | NetError::Connect { .. }),
            ) => return Err(io_err(e)),
        };
        let closer: Arc<dyn Closer> = Arc::new(SocketCloser(sock.try_clone()?));
        closer_slot(Arc::clone(&closer));
        let left = deadline.left().ok_or(SmpError::Timeout)?;
        let cut = Arc::clone(&closer);
        let dog = Watchdog::arm(left, move || cut.close())?;
        let settled = handshake(sock, server, cfg);
        if dog.disarm() {
            return Err(SmpError::Timeout);
        }
        let (rd, wr, session) = settled?;
        Ok(Wire {
            reader: Box::new(rd),
            writer: Box::new(wr),
            closer,
            session,
        })
    }
}

/// TLS, then the SMP handshake over it.
fn handshake(
    sock: TcpStream,
    server: &SmpServer,
    cfg: &ConnConfig,
) -> Result<(Rd, Wr, Established), SmpError> {
    let pin = Pin {
        key_hash: server.key_hash.clone(),
        now: (cfg.wall_clock)(),
    };
    let tls = TlsStream::connect_pinned(sock, &["smp/1"], pin).map_err(tls_failure)?;
    let (tls_unique, peer_chain) = (*tls.tls_unique(), tls.peer_cert_chain().to_vec());
    let (r, w) = tls.split();
    let (mut rd, mut wr) = (Rd(r), Wr(w));
    let facts = TlsFacts {
        tls_unique: &tls_unique,
        peer_chain: &peer_chain,
    };
    let hs = HandshakeConfig {
        range: cfg.versions,
        block_encryption: cfg.block_encryption,
        timeout: cfg.connect_timeout,
        now: (cfg.wall_clock)(),
    };
    let session = client_handshake(&mut rd, &mut wr, &facts, &server.key_hash, &hs)?;
    Ok((rd, wr, session))
}
