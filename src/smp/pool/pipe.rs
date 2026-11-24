//! The byte pipe to the tunnel, as the pool sees it: a WebSocket in a browser, a fake in tests.
use super::super::error::SmpError;

/// Names one dialled pipe; what it reports carries this, so news of a replaced pipe is ignored.
pub type PipeId = u64;

/// What a pipe reports, in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeInput {
    Open,
    Text(String),
    Binary(Vec<u8>),
    /// The pipe ended; `code` and `reason` as the WebSocket close gave them.
    Closed {
        code: u16,
        reason: String,
    },
}

pub trait Pipe {
    fn send_text(&mut self, text: &str) -> Result<(), SmpError>;
    fn send_binary(&mut self, bytes: &[u8]) -> Result<(), SmpError>;
    /// Ends the pipe; it reports nothing after this.
    fn close(&mut self);
}

pub trait Dialer {
    /// Opens a pipe to the tunnel. What it reports reaches `TunnelPool::on_input` under `id`.
    fn dial(&mut self, id: PipeId) -> Result<Box<dyn Pipe>, SmpError>;
}
