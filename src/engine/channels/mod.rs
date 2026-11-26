//! Channels: broadcast groups whose owners post through chat relays, owner and subscriber sides (simplex-chat 7.1.4 Commands.hs `APINewPublicGroup`, `APIConnectPreparedGroup`; Subscriber.hs relay handling; Internal.hs signing).

mod accept;
mod create;
mod inbound;
mod invite;
mod join;
mod keys;
mod lookup;
mod post_in;
mod post_out;
mod publish;
mod relay_conn;
mod relay_events;
mod relay_test;
mod relay_test_end;
mod relays_config;
mod roster;
mod status;
mod subscribe;
pub(crate) mod verify;
