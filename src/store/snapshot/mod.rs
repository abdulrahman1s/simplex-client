//! A store as one byte string, authenticated as a whole by a MAC under the store key, and the deltas saved after it.

mod chain;
mod codec;
mod delta;
mod export;

pub(super) use delta::{Link, Written};

#[cfg(test)]
pub(super) use codec::{decode, encode};
