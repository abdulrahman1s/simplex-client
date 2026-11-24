//! A store as one byte string, authenticated as a whole by a MAC under the store key.

mod codec;
mod export;

#[cfg(test)]
pub(super) use codec::{decode, encode};
