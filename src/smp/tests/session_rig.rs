//! Moving bytes by hand between a sans-I/O session and the SMP server behind the TLS test server.
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

use super::support::private;
use super::tls_server::{Respond, Served};
use crate::crypto::Alg;
use crate::smp::{Command, ConnConfig, Request, Response, SmpError, SmpEventKind, SmpSession};

const WAIT: Duration = Duration::from_secs(5);
pub const T0: u64 = 1_000;

pub struct Client {
    pub s: SmpSession,
    pub served: Served,
    /// Bytes per `feed` call.
    step: usize,
    pub events: Vec<SmpEventKind>,
}

impl Client {
    pub fn start(cfg: &ConnConfig, served: Served, step: usize) -> Self {
        let (s, hello) = SmpSession::start(&served.server, cfg, T0).unwrap();
        let mut c = Self {
            s,
            served,
            step,
            events: Vec::new(),
        };
        c.served.sock.write_all(&hello).unwrap();
        c
    }

    pub fn flush(&mut self) {
        while let Some(out) = self.s.take_outgoing() {
            self.served.sock.write_all(&out).unwrap();
        }
    }

    /// Sends what is queued, then reads once from the server and feeds it.
    pub fn pump(&mut self, now: u64) {
        self.flush();
        let mut buf = [0u8; 8192];
        let n = self.served.sock.read_within(&mut buf, WAIT).unwrap();
        assert!(n > 0, "the server closed the pipe");
        for chunk in buf[..n].chunks(self.step) {
            self.events.extend(self.s.feed(chunk, now));
        }
        self.flush();
    }

    pub fn until(&mut self, now: u64, want: impl Fn(&SmpEventKind) -> bool) -> SmpEventKind {
        loop {
            if let Some(i) = self.events.iter().position(&want) {
                return self.events.remove(i);
            }
            self.pump(now);
        }
    }

    pub fn up(&mut self) -> SmpEventKind {
        self.until(T0, |e| {
            matches!(e, SmpEventKind::Up(_) | SmpEventKind::ConnectFailed(_))
        })
    }

    pub fn reply(&mut self, now: u64, tag: u64) -> Result<Response, SmpError> {
        let got = self.until(
            now,
            |e| matches!(e, SmpEventKind::Reply { tag: t, .. } if *t == tag),
        );
        let SmpEventKind::Reply { answer, .. } = got else {
            unreachable!()
        };
        answer
    }

    /// Waits until the server has received `n` transmissions.
    pub fn server_saw(&self, n: usize) {
        for _ in 0..500 {
            if self.served.seen.lock().unwrap().len() >= n {
                return;
            }
            sleep(Duration::from_millis(10));
        }
        panic!("the server did not receive {n} transmissions");
    }
}

pub fn quiet() -> ConnConfig {
    ConnConfig {
        ping_interval: Duration::ZERO,
        ..ConnConfig::default()
    }
}

pub fn answer_with(r: Response) -> Respond {
    Box::new(move |t| vec![(t.corr_id.clone(), t.entity_id.clone(), r.clone())])
}

pub fn sub(entity: &[u8]) -> Request {
    Request::new(Command::Sub, entity.to_vec(), Some(private(Alg::Ed25519)))
}
