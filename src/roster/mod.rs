//! The chat list the high-level clients keep, and the engine's events lifted into the ones they report.

mod cache;
mod lift;

#[cfg(test)]
mod tests;

pub(crate) use cache::Roster;
pub(crate) use lift::{Note, lift};
