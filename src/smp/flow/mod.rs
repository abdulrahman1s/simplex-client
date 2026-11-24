//! The pure core of the command flow both drivers share: requests, waiters, liveness, routing, back-off (simplexmq Client.hs).

mod backoff;
mod liveness;
mod pending;
mod request;
mod route;

pub use backoff::Backoff;
pub use liveness::{Liveness, Verdict};
pub use pending::{Entry, Pending};
pub use request::{Answer, Request};
pub use route::route;
