//! The native chat engine: one thread owning state, store and agent, ported from
//! simplex-chat's `Library/Commands.hs` and `Library/Subscriber.hs` (AGPL-3.0-only).

mod address;
pub mod agent_adapter;
mod agent_map;
pub mod agent_port;
mod calls;
mod chats;
pub mod clock;
mod data;
pub mod error;
pub(crate) mod files;
mod groups;
mod inbound;
mod items;
pub mod jobs;
mod link;
mod load;
mod local;
pub mod log;
pub mod machine;
mod prefs;
mod presets;
mod profile;
mod quote;
pub mod records;
pub mod relays;
mod requests;
#[cfg(feature = "native")]
pub mod run;
mod send_tags;
mod start;
pub mod state;
mod time;
mod timed;

#[cfg(test)]
#[cfg(feature = "native")]
pub(crate) mod tests;

pub use agent_adapter::NativeAgent;
pub use agent_port::{AgentEvent, AgentFailure, AgentMsgId, AgentPort, ConnId};
pub use clock::{Clock, FakeClock, SystemClock};
pub use machine::Engine;
#[cfg(feature = "native")]
pub(crate) use requests::kind_of;
