//! What goes through the command queue to the writer thread.
use crossbeam_channel::Sender;

use crate::smp::error::SmpError;
use crate::smp::responses::Response;

use crate::smp::flow::Request;

/// Where a command's answer goes: its own channel (`send`), or the event channel under the caller's tag (`send_tagged`).
#[derive(Debug, Clone)]
pub enum Reply {
    Channel(Sender<Result<Response, SmpError>>),
    Tagged(u64),
}

pub struct Outgoing {
    pub request: Request,
    pub reply: Reply,
}

pub enum Ctl {
    Close,
    /// The reader of session `epoch` stopped.
    ReaderDone {
        epoch: u64,
        err: SmpError,
    },
}
