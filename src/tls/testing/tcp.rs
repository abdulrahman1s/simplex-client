//! Loopback TCP servers for connect deadlines: one that drips middlebox records, one that bridges to the mock TLS server.
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use super::options::MockOpts;
use super::pipe::Pipe;
use super::server::spawn_server;
use crate::net::Socket;

pub(crate) const DRIP: Duration = Duration::from_millis(100);

/// Answers every ClientHello with a ChangeCipherSpec record each `DRIP`, which is legal and never
/// ends the handshake. Returns the port and how many connections it has taken.
pub(crate) fn dripping_server() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let taken = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&taken);
    thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            count.fetch_add(1, Ordering::AcqRel);
            thread::spawn(move || {
                let _ = s.read(&mut [0u8; 4096]);
                while s.write_all(&[20, 3, 3, 0, 1, 1]).is_ok() {
                    thread::sleep(DRIP);
                }
            });
        }
    });
    (port, taken)
}

/// The mock TLS server behind a loopback port, for one connection.
pub(crate) fn tls_server(opts: MockOpts) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let (near, far) = Pipe::pair(0);
        let _server = spawn_server(far, opts);
        let (mut tcp_in, mut pipe_out) = (tcp.try_clone().unwrap(), near.try_clone().unwrap());
        thread::spawn(move || copy(&mut tcp_in, &mut pipe_out));
        let (mut pipe_in, mut tcp_out) = (near, tcp);
        copy(&mut pipe_in, &mut tcp_out);
    });
    port
}

fn copy(from: &mut impl Read, to: &mut (impl Write + Ends)) {
    let mut buf = [0u8; 8192];
    while let Ok(n @ 1..) = from.read(&mut buf) {
        if to.write_all(&buf[..n]).is_err() {
            break;
        }
    }
    to.end();
}

trait Ends {
    fn end(&self);
}

impl Ends for TcpStream {
    fn end(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

impl Ends for Pipe {
    fn end(&self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}
