//! A tunnel pool whose pipes are recorded by the test, which carries their bytes to the TLS test server by hand.
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::time::Duration;

use crossbeam_channel::{Receiver, unbounded};

use super::session_rig::quiet;
use super::tls_server::Served;
use crate::smp::{
    ConnConfig, Dialer, Pipe, PipeId, PipeInput, SmpError, SmpEvent, SmpEventKind, TunnelPool,
};

pub const T0: u64 = 1_000;

/// What the pool did to its pipes, in order.
#[derive(Default)]
pub struct Wire {
    pub dialled: Vec<PipeId>,
    pub texts: Vec<String>,
    /// Binary frames not yet carried to the server.
    pub frames: Vec<Vec<u8>>,
    pub largest_frame: usize,
    pub closed: Vec<PipeId>,
    pub targets: Vec<String>,
}

struct FakePipe(PipeId, Rc<RefCell<Wire>>);

impl Pipe for FakePipe {
    fn send_text(&mut self, text: &str) -> Result<(), SmpError> {
        self.1.borrow_mut().texts.push(text.to_owned());
        Ok(())
    }

    fn send_binary(&mut self, bytes: &[u8]) -> Result<(), SmpError> {
        let mut w = self.1.borrow_mut();
        w.largest_frame = w.largest_frame.max(bytes.len());
        w.frames.push(bytes.to_vec());
        Ok(())
    }

    fn close(&mut self) {
        self.1.borrow_mut().closed.push(self.0);
    }
}

/// `.1`: through the tunnel, or straight to the relay.
struct FakeDialer(Rc<RefCell<Wire>>, bool);

impl Dialer for FakeDialer {
    fn dial(&mut self, id: PipeId, host: &str, port: u16) -> Result<Box<dyn Pipe>, SmpError> {
        let mut w = self.0.borrow_mut();
        w.dialled.push(id);
        w.targets.push(format!("{host}:{port}"));
        Ok(Box::new(FakePipe(id, Rc::clone(&self.0))))
    }

    fn tunnelled(&self) -> bool {
        self.1
    }
}

pub struct Rig {
    pub pool: TunnelPool,
    pub wire: Rc<RefCell<Wire>>,
    pub events: Receiver<SmpEvent>,
}

impl Rig {
    pub fn new() -> Self {
        Self::with(quiet())
    }

    /// Pipes straight to the relay, as direct TCP on node.
    pub fn direct() -> Self {
        Self::build(quiet(), false)
    }

    pub fn with(cfg: ConnConfig) -> Self {
        Self::build(cfg, true)
    }

    fn build(cfg: ConnConfig, tunnel: bool) -> Self {
        let wire = Rc::new(RefCell::new(Wire::default()));
        let (tx, events) = unbounded();
        let dialer = Box::new(FakeDialer(Rc::clone(&wire), tunnel));
        let pool = TunnelPool::new(cfg, Some("tok".into()), dialer, tx);
        Self { pool, wire, events }
    }

    pub fn last_pipe(&self) -> PipeId {
        *self.wire.borrow().dialled.last().expect("a dial")
    }

    pub fn events(&self) -> Vec<SmpEventKind> {
        self.events.try_iter().map(|e| e.kind).collect()
    }

    /// Opens the newest pipe and answers its hello with `{"ok":true}`.
    pub fn open_tunnel(&mut self, now: u64) -> PipeId {
        let id = self.last_pipe();
        self.pool.on_input(id, PipeInput::Open, now);
        self.pool
            .on_input(id, PipeInput::Text(r#"{"ok":true}"#.into()), now);
        id
    }

    /// Carries the frames to the server, then one read of its answer back into the pool.
    pub fn pump(&mut self, served: &mut Served, id: PipeId, now: u64) {
        self.pool.flush(now);
        for frame in std::mem::take(&mut self.wire.borrow_mut().frames) {
            served.sock.write_all(&frame).unwrap();
        }
        let mut buf = vec![0u8; 70_000];
        let n = (served.sock)
            .read_within(&mut buf, Duration::from_secs(5))
            .unwrap();
        assert!(n > 0, "the server closed the pipe");
        self.pool
            .on_input(id, PipeInput::Binary(buf[..n].to_vec()), now);
        self.pool.flush(now);
    }

    /// Pumps until an event `want` accepts arrives; the others are kept in `seen`.
    pub fn until(
        &mut self,
        served: &mut Served,
        id: PipeId,
        now: u64,
        seen: &mut Vec<SmpEventKind>,
        want: impl Fn(&SmpEventKind) -> bool,
    ) -> SmpEventKind {
        for _ in 0..50 {
            seen.extend(self.events());
            if let Some(i) = seen.iter().position(&want) {
                return seen.remove(i);
            }
            self.pump(served, id, now);
        }
        panic!("the event did not come: {seen:?}");
    }
}
