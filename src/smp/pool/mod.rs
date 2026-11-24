//! Relays through a WebSocket byte tunnel, and the pipes files use too: one sans-I/O `SmpSession` per relay over a pipe the owner dials, reconnecting with back-off (the Worker's tunnel protocol v1).
mod carrier;
mod dial;
mod ending;
mod link;
mod pipe;
mod slot;
mod tunnel_pool;
mod why;

pub use carrier::Carrier;
pub use pipe::{Dialer, Pipe, PipeId, PipeInput};
pub use tunnel_pool::{MAX_PIPES, TunnelPool};

/// The pool as the agent and the runtime that feeds it both hold it; one thread only.
pub type SharedPool = std::rc::Rc<std::cell::RefCell<TunnelPool>>;
