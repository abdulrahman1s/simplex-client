//! Moving bytes by hand between a sans-I/O session and the in-process server.
use std::io::Write;
use std::time::Duration;

use super::fake_server::Fake;
use crate::crypto::KeyPair;
use crate::h2;
use crate::tls::testing::Pipe;
use crate::xftp::{ClientConfig, Command, Response, SessionEvent, XftpError, XftpSession};

pub const T0: u64 = 1_000;
const WAIT: Duration = Duration::from_secs(5);

pub struct Rig {
    pub s: XftpSession,
    sock: Pipe,
    events: Vec<SessionEvent>,
}

impl Rig {
    pub fn start(fake: &Fake, send_ahead: u32) -> Self {
        let (mut sock, _server) = fake.serve(0);
        let opts = h2::Options {
            send_ahead,
            ..h2::Options::default()
        };
        let cfg = ClientConfig::default();
        let (s, hello) = XftpSession::start(&fake.address, "files.test", cfg, opts, T0).unwrap();
        sock.write_all(&hello).unwrap();
        Self {
            s,
            sock,
            events: Vec::new(),
        }
    }

    fn flush(&mut self) {
        while let Some(out) = self.s.take_outgoing() {
            self.sock.write_all(&out).unwrap();
        }
    }

    pub fn pump(&mut self, now: u64) {
        self.flush();
        let mut buf = vec![0u8; 65536];
        let n = self.sock.read_within(&mut buf, WAIT).unwrap();
        assert!(n > 0, "the server closed the pipe");
        self.events.extend(self.s.feed(&buf[..n], now));
        self.flush();
    }

    pub fn next(&mut self, now: u64) -> SessionEvent {
        while self.events.is_empty() {
            self.pump(now);
        }
        self.events.remove(0)
    }

    pub fn call(
        &mut self,
        key: &KeyPair,
        entity: &[u8],
        cmd: &Command,
        file: &[u8],
        max: usize,
    ) -> Result<(Response, Vec<u8>), XftpError> {
        self.s
            .send(Some(&key.private), entity, cmd, file, max, T0)
            .unwrap();
        match self.next(T0) {
            SessionEvent::Answer(a) => a,
            other => panic!("expected an answer, got {other:?}"),
        }
    }
}
