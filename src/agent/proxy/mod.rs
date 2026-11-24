//! Private message routing for the agent: sender commands to other people's relays go through a relay of ours (Agent/Client.hs `sendOrProxySMPCommand`, simplexmq 7.1.4, AGPL-3.0-only).
mod answer;
mod choose;
mod send;
mod state;

pub(crate) use state::Proxies;
