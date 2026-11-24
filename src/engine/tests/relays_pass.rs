//! The relay testers of test engines: each relay passes, so no test reaches the presets on the network. `Passing` is handed in on the next `T::out`; `Answering`, the fake agent's, answers on the jobs channel for a run loop.

use std::sync::{Arc, Mutex, PoisonError};

use crossbeam_channel::Sender;

use super::FakeAgent;
use crate::engine::clock::FakeClock;
use crate::engine::jobs::JobDone;
use crate::engine::log::Log;
use crate::engine::machine::Engine;
use crate::engine::relays::{RelayDone, RelayTester};
use crate::store::Store;

/// Answers not yet handed in; bounded by one check's relays, emptied by `take`.
#[derive(Clone, Default)]
pub struct Passing(Arc<Mutex<Vec<RelayDone>>>);

impl Passing {
    pub fn take(&self) -> Vec<RelayDone> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl RelayTester for Passing {
    fn test(&mut self, server: String, run: u64, _: Sender<JobDone>) {
        let done = RelayDone {
            run,
            server,
            failure: None,
        };
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(done);
    }
}

/// Answers every test at once as passed, on the channel a run loop reads.
pub struct Answering;

impl RelayTester for Answering {
    fn test(&mut self, server: String, run: u64, tx: Sender<JobDone>) {
        let done = RelayDone {
            run,
            server,
            failure: None,
        };
        let _ = tx.send(JobDone::Relay(done)); // the engine is gone
    }
}

/// A test engine on `store` whose relay checks all pass.
pub fn engine(store: Store, clock: &FakeClock) -> (Engine<FakeAgent>, Passing) {
    let mut e = Engine::new(
        store,
        FakeAgent::default(),
        Box::new(clock.clone()),
        Log::discard(),
    );
    let passing = Passing::default();
    e.relays.tester = Some(Box::new(passing.clone()));
    (e, passing)
}
