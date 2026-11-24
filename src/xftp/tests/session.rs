//! The sans-I/O session against the in-process server, bytes moved by hand.
use std::io::Write;
use std::time::Duration;

use super::fake_server::Fake;
use crate::crypto::{Alg, KeyPair, random_vec, sha256};
use crate::h2;
use crate::tls::testing::Pipe;
use crate::xftp::{
    ClientConfig, Command, FileInfo, Response, SessionEvent, XftpError, XftpErrorType, XftpSession,
    fetched_len, open_fetched,
};

const T0: u64 = 1_000;
const WAIT: Duration = Duration::from_secs(5);

struct Rig {
    s: XftpSession,
    sock: Pipe,
    events: Vec<SessionEvent>,
}

impl Rig {
    fn start(fake: &Fake, send_ahead: u32) -> Self {
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

    fn pump(&mut self, now: u64) {
        self.flush();
        let mut buf = vec![0u8; 65536];
        let n = self.sock.read_within(&mut buf, WAIT).unwrap();
        assert!(n > 0, "the server closed the pipe");
        self.events.extend(self.s.feed(&buf[..n], now));
        self.flush();
    }

    fn next(&mut self, now: u64) -> SessionEvent {
        while self.events.is_empty() {
            self.pump(now);
        }
        self.events.remove(0)
    }

    fn call(
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

fn ready(fake: &Fake, send_ahead: u32) -> Rig {
    let mut rig = Rig::start(fake, send_ahead);
    assert!(matches!(rig.next(T0), SessionEvent::Ready));
    assert_eq!(rig.s.version(), Some(4));
    rig
}

#[test]
fn a_chunk_goes_up_in_paced_frames_and_comes_back_through_the_session() {
    let fake = Fake::new();
    let mut rig = ready(&fake, 100_000);
    let chunk = random_vec(256 * 1024).unwrap();
    let (snd, rcv) = (
        KeyPair::generate(Alg::Ed25519).unwrap(),
        KeyPair::generate(Alg::Ed25519).unwrap(),
    );
    let info = FileInfo {
        snd_key: snd.public.clone(),
        size: chunk.len() as u32,
        digest: sha256(&chunk).to_vec(),
    };
    let new = Command::New {
        info,
        rcv_keys: vec![rcv.public.clone()],
        basic_auth: None,
        storage_hours: None,
    };
    let (sids, _) = rig.call(&snd, &[], &new, &[], 0).unwrap();
    let Response::SndIds {
        sender_id,
        recipient_ids,
        ..
    } = sids
    else {
        panic!("{sids:?}")
    };
    let (ok, _) = rig
        .call(&snd, &sender_id, &Command::Put, &chunk, 0)
        .unwrap();
    assert_eq!(ok, Response::Ok);
    assert!(rig.s.progress().sent >= chunk.len(), "the whole body went");
    let dh = KeyPair::generate(Alg::X25519).unwrap();
    let get = Command::Get {
        rcv_dh_key: dh.public.clone(),
    };
    let max = fetched_len(chunk.len() as u32).unwrap();
    let (file, body) = rig.call(&rcv, &recipient_ids[0], &get, &[], max).unwrap();
    let got = open_fetched(&dh.private, file, body, chunk.len() as u32, &sha256(&chunk)).unwrap();
    assert_eq!(got, chunk);
    assert!(rig.s.is_idle());
}

#[test]
fn a_server_error_answers_the_command_and_the_session_stays() {
    let fake = Fake::new();
    let mut rig = ready(&fake, u32::MAX >> 1);
    let key = KeyPair::generate(Alg::Ed25519).unwrap();
    let dh = KeyPair::generate(Alg::X25519).unwrap();
    let get = Command::Get {
        rcv_dh_key: dh.public,
    };
    let err = rig.call(&key, b"nobody", &get, &[], 100).unwrap_err();
    assert!(
        matches!(err, XftpError::Server(XftpErrorType::Auth)),
        "{err:?}"
    );
    assert!(rig.s.is_idle());
    let (pong, _) = rig.call(&key, &[], &Command::Ping, &[], 0).unwrap();
    assert_eq!(pong, Response::Pong);
}

#[test]
fn a_server_with_another_identity_is_refused_at_the_handshake() {
    let fake = Fake::new();
    let mut other = Fake::new();
    other.address = fake.address.clone();
    let mut rig = Rig::start(&other, u32::MAX >> 1);
    let ev = rig.next(T0);
    assert!(
        matches!(ev, SessionEvent::Closed(XftpError::Handshake(_))),
        "{ev:?}"
    );
    assert!(rig.s.is_down());
}

#[test]
fn a_command_past_its_deadline_ends_the_session() {
    let fake = Fake::new();
    let mut rig = ready(&fake, u32::MAX >> 1);
    let key = KeyPair::generate(Alg::Ed25519).unwrap();
    rig.s.send(None, &[], &Command::Ping, &[], 0, T0).unwrap();
    let due = rig.s.next_deadline().unwrap();
    assert_eq!(due, T0 + ClientConfig::default().command_budget_ms(0));
    assert!(rig.s.tick(due).is_empty(), "not before the deadline");
    let ev = rig.s.tick(due + 1);
    assert!(
        matches!(
            ev.as_slice(),
            [SessionEvent::Answer(Err(XftpError::TimedOut))]
        ),
        "{ev:?}"
    );
    assert!(rig.s.is_down());
    assert!(
        rig.s
            .send(Some(&key.private), &[], &Command::Ping, &[], 0, T0)
            .is_err()
    );
}
