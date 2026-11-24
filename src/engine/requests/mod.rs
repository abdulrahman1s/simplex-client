//! One file per request group; `dispatch` is the table.

mod change;
mod codes;
mod connect;
mod delete;
mod dispatch;
mod history;
mod invite;
mod kind;
mod local;
mod picture;
mod profile;
mod react;
mod read;
mod remove;
mod send;
mod size;

pub(crate) use change::has_content;
#[cfg(feature = "native")]
pub(crate) use kind::kind_of;
pub(crate) use size::sized;
