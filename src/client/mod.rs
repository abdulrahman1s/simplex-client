//! The high-level client: start it, read its events, call its methods.

mod answers;
mod builder;
mod dispatch;
mod error;
mod groups;
mod handle;
mod messages;
mod people;
mod spawn;
mod waiters;

#[cfg(test)]
mod tests;

pub use builder::ClientBuilder;
pub use error::{Error, Result};
pub use handle::Client;
