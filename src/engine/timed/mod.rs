//! Disappearing messages (Messages.hs `CITimed`; Internal.hs `startTimedItemThread`, `deleteTimedItem`).

pub(crate) mod adopt;
mod expire;
pub(crate) mod index;
mod keys;
mod notice;
mod read;
pub(crate) mod rules;
mod send;
mod set;
