//! What the client object, its sockets and its timer share, and the one loop that runs inputs.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use super::handlers::Handlers;
use super::outcome::Outcome;
use super::runtime::Core;
use super::timer::Timer;
use crate::smp::{PipeId, PipeInput};
use crate::types::Request;

/// One per client. Sockets and the timer hold it weakly, so dropping the client frees it.
pub(super) struct Shared {
    /// `None` once closed.
    pub(super) core: RefCell<Option<Core>>,
    /// What the sockets reported. Eviction: emptied by every run, which each report starts.
    pub(super) inbox: RefCell<VecDeque<(PipeId, PipeInput)>>,
    /// Requests from JS calls. Eviction: emptied by every run, which each call starts.
    pub(super) requests: RefCell<VecDeque<Request>>,
    pub(super) handlers: RefCell<Handlers>,
    pub(super) timer: RefCell<Option<Timer>>,
    /// A run is under way: a JS handler calling the client must not start another inside it.
    busy: Cell<bool>,
    again: Cell<bool>,
}

impl Shared {
    pub(super) fn new() -> Rc<Self> {
        Rc::new(Self {
            core: RefCell::new(None),
            inbox: RefCell::new(VecDeque::new()),
            requests: RefCell::new(VecDeque::new()),
            handlers: RefCell::new(Handlers::default()),
            timer: RefCell::new(None),
            busy: Cell::new(false),
            again: Cell::new(false),
        })
    }

    /// A socket's report: handled now, or by the run under way.
    pub(super) fn post(self: &Rc<Self>, id: PipeId, input: PipeInput) {
        self.inbox.borrow_mut().push_back((id, input));
        self.run();
    }

    pub(super) fn request(self: &Rc<Self>, request: Request) {
        self.requests.borrow_mut().push_back(request);
        self.run();
    }

    /// Runs every waiting input to completion, then tells JS, then again while JS handlers
    /// gave it more; then sets the timer for the next deadline. No borrow is held while JS runs.
    pub(super) fn run(self: &Rc<Self>) {
        if self.busy.replace(true) {
            self.again.set(true);
            return;
        }
        loop {
            self.again.set(false);
            let inbox = std::mem::take(&mut *self.inbox.borrow_mut());
            let requests = std::mem::take(&mut *self.requests.borrow_mut());
            let outcome = match self.core.borrow_mut().as_mut() {
                Some(core) => core.run(inbox, requests),
                None => Outcome::default(),
            };
            outcome.deliver(&self.handlers);
            if !self.again.get() {
                break;
            }
        }
        self.busy.set(false);
        self.arm();
    }

    fn arm(&self) {
        let wait = self.core.borrow().as_ref().map(Core::wait_ms);
        if let (Some(ms), Some(timer)) = (wait, self.timer.borrow().as_ref()) {
            timer.arm(ms);
        }
    }

    /// Ends the client: the network closes, waiting calls fail, the timer stops.
    pub(super) fn close(&self) {
        let core = self.core.borrow_mut().take();
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.stop();
        }
        if let Some(core) = core {
            core.close().deliver(&self.handlers);
        }
        self.inbox.borrow_mut().clear();
        self.requests.borrow_mut().clear();
    }
}
