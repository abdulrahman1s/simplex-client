//! Files in the wasm runtime: the in-memory store, the transfer runner on the runtime's timer, and the calls waiting on files.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Weak;

use super::options::Transport;
use super::shared::Shared;
use super::socket::WsDialer;
use super::tcp::TcpDialer;
use crate::engine::files::{DEFAULT_LIMIT, FileRunner, MemFiles, queue};
use crate::engine::{Engine, NativeAgent};
use crate::smp::{Dialer, PipeId, PipeInput};
use crate::types::model::ChatRef;

/// A promise's two functions.
pub(super) type Parts = (js_sys::Function, js_sys::Function);

/// A `sendFile` call waiting for the message its file went out with.
pub(super) struct Sending {
    pub(super) path: PathBuf,
    /// The tag its request went with, which a refusal is told against.
    pub(super) tag: u64,
    pub(super) parts: Parts,
}

/// A `download` call waiting for the file's bytes.
pub(super) struct Receiving {
    pub(super) chat: ChatRef,
    pub(super) item: i64,
    /// The engine started the download, so a refusal is not this call's.
    pub(super) started: bool,
    pub(super) parts: Parts,
}

pub(super) struct Files {
    pub(super) mem: MemFiles,
    runner: FileRunner,
    /// Eviction: settled by its message, by its request's refusal, or at close.
    pub(super) sending: VecDeque<Sending>,
    /// Eviction: settled when the file arrives, fails or is cancelled, or at close.
    pub(super) receiving: Vec<Receiving>,
}

impl Files {
    /// Gives the engine memory for files and a queue its transfers go to; the runner dials
    /// file servers the way relays are reached: the tunnel, or TCP on Node.js and Bun.
    pub(super) fn attach(
        engine: &mut Engine<NativeAgent>,
        transport: &Transport,
        token: Option<String>,
        shared: Weak<Shared>,
    ) -> Self {
        let dialer: Box<dyn Dialer> = match transport {
            Transport::Tunnel(url) => Box::new(WsDialer {
                url: url.clone(),
                shared,
            }),
            Transport::Tcp => Box::new(TcpDialer { shared }),
        };
        let mem = MemFiles::new(DEFAULT_LIMIT);
        let (transfers, jobs) = queue();
        engine.files.transfers = Box::new(transfers);
        engine.files.store = Box::new(mem.clone());
        Self {
            runner: FileRunner::new(jobs, dialer, token, mem.clone()),
            mem,
            sending: VecDeque::new(),
            receiving: Vec::new(),
        }
    }

    /// Whether pipe `id` carries a file transfer rather than a relay.
    pub(super) fn owns(id: PipeId) -> bool {
        FileRunner::owns(id)
    }

    pub(super) fn on_input(&mut self, id: PipeId, input: PipeInput, now: u64) {
        self.runner.on_input(id, input, now);
    }

    /// New transfers start, timers fire, bytes go out.
    pub(super) fn pump(&mut self, now: u64) {
        self.runner.tick(now);
        self.runner.flush(now);
    }

    /// In the clock `pump` takes.
    pub(super) fn next_deadline(&self) -> Option<u64> {
        self.runner.next_deadline()
    }

    /// Transfers stop and every waiting call fails with `closed`.
    pub(super) fn close(&mut self) {
        self.runner.close();
        let closed = super::convert::closed();
        let parts = (self.sending.drain(..).map(|s| s.parts))
            .chain(self.receiving.drain(..).map(|r| r.parts));
        for (_, reject) in parts {
            let _ = reject.call1(&wasm_bindgen::JsValue::NULL, &closed); // a promise's own functions do not throw
        }
    }
}
