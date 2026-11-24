//! The sans-I/O path against a real xftp-server: `MemUpload` and `MemDownload` over an
//! `XftpSession` on a plain TCP socket. Run with `SIMPLEX_TEST_XFTP=xftp://…@127.0.0.1:<port>
//! cargo test --test it xftp_live_sans_io -- --ignored`.
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use simplex_client::crypto::random_vec;
use simplex_client::h2;
use simplex_client::smp::{Clock, SystemClock};
use simplex_client::xftp::{
    Answer, ClientConfig, MemDownload, MemUpload, SessionEvent, Step, XftpServer, XftpSession,
};

fn server() -> XftpServer {
    let Ok(addr) = std::env::var("SIMPLEX_TEST_XFTP") else {
        panic!("SIMPLEX_TEST_XFTP is not set")
    };
    addr.parse().unwrap()
}

/// A session on a socket, driven until it has something to say.
struct Live {
    s: XftpSession,
    sock: TcpStream,
    clock: SystemClock,
}

impl Live {
    fn open(server: &XftpServer) -> Self {
        let host = server.dialable_hosts().next().unwrap();
        let sock = TcpStream::connect((host, server.port_or_default())).unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let clock = SystemClock::new();
        let opts = h2::Options {
            send_ahead: 256 * 1024,
            ..h2::Options::default()
        };
        let (s, hello) =
            XftpSession::start(server, host, ClientConfig::default(), opts, clock.now_ms())
                .unwrap();
        let mut live = Self { s, sock, clock };
        live.sock.write_all(&hello).unwrap();
        assert!(matches!(live.next(), SessionEvent::Ready));
        live
    }

    fn next(&mut self) -> SessionEvent {
        let mut buf = vec![0u8; 65536];
        loop {
            while let Some(out) = self.s.take_outgoing() {
                self.sock.write_all(&out).unwrap();
            }
            let now = self.clock.now_ms();
            let mut events = match self.sock.read(&mut buf) {
                Ok(0) => panic!("the server closed the socket"),
                Ok(n) => self.s.feed(&buf[..n], now),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    self.s.tick(now)
                }
                Err(e) => panic!("{e}"),
            };
            if !events.is_empty() {
                return events.remove(0);
            }
        }
    }

    fn run(&mut self, call: &simplex_client::xftp::Call) -> Answer {
        let now = self.clock.now_ms();
        self.s
            .send(
                call.key.as_ref(),
                &call.entity,
                &call.cmd,
                &call.file,
                call.max_file,
                now,
            )
            .unwrap();
        match self.next() {
            SessionEvent::Answer(a) => a,
            other => panic!("{other:?}"),
        }
    }
}

#[test]
#[ignore = "needs a local xftp-server; set SIMPLEX_TEST_XFTP"]
fn a_3_mb_file_goes_up_and_down_through_the_sans_io_session() {
    let server = server();
    let content = random_vec(3_000_000).unwrap();
    let mut live = Live::open(&server);
    println!("negotiated XFTP v{:?}", live.s.version());
    let mut up = MemUpload::new(&content, "three.bin", &[server], 2, None).unwrap();
    let uploaded = loop {
        match up.next_step().unwrap() {
            Step::Call(call) => {
                let a = live.run(&call);
                up.on_answer(a).unwrap();
            }
            Step::Done(u) => break u,
        }
    };
    assert!(
        uploaded.recipients[1].chunks.len() > 1,
        "more than one chunk"
    );
    let mut down = MemDownload::new(&uploaded.recipients[1], 64 << 20).unwrap();
    let got = loop {
        match down.next_step().unwrap() {
            Step::Call(call) => {
                let a = live.run(&call);
                down.on_answer(a).unwrap();
            }
            Step::Done(f) => break f,
        }
    };
    assert_eq!(got.file_name, "three.bin");
    assert!(got.bytes == content, "the bytes came back whole");
}
