//! What the engine needs from the agent layer, mirroring simplexmq's Agent API (Agent.hs, AGPL-3.0-only).

use crate::types::{ProxyFallback, ProxyMode};
use serde::{Deserialize, Serialize};

use crate::agent::links::ConnectionLink;
use crate::store::Store;

/// A channel's link data made from its own short link (`create_channel_link`).
pub type LinkUserData = Box<dyn FnOnce(&str) -> Vec<u8>>;

/// The agent's connection number (`agent::client::ConnId`), never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConnId(pub i64);

/// simplexmq's `AgentMsgId`: per connection, increasing.
pub type AgentMsgId = i64;

/// Why an agent call or delivery failed, as far as the engine tells them apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentFailure {
    /// A relay did not answer (network, timeout, host): `SX-CHT-004`.
    Network(String),
    /// The link was refused or could not be used: `SX-CHT-005` for a connection.
    BadLink(String),
    Other(String),
}

impl AgentFailure {
    pub fn detail(&self) -> &str {
        match self {
            Self::Network(d) | Self::BadLink(d) | Self::Other(d) => d,
        }
    }
}

/// The agent's events (`ACommand` events in Agent/Protocol.hs) the engine acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEvent {
    /// The link of a `create_invitation`: the full one and the short one when the relay stores it.
    Invitation {
        conn: ConnId,
        full: String,
        short: Option<String>,
    },
    /// `CONF`: the joining side's conn-info arrived on our invitation; answer with `allow`.
    Conf {
        conn: ConnId,
        conn_info: Vec<u8>,
    },
    /// `INFO`: the inviting side's conn-info arrived on a link we joined.
    Info {
        conn: ConnId,
        conn_info: Vec<u8>,
    },
    /// `CON`: both directions work.
    Con {
        conn: ConnId,
    },
    /// `MSG`: must be acknowledged with `ack`, whatever the engine makes of it. `integrity`
    /// says how it followed the message before it (`MsgMeta.integrity`); `broker_ts` is when
    /// the relay took it, in seconds.
    Msg {
        conn: ConnId,
        msg_id: AgentMsgId,
        body: Vec<u8>,
        integrity: crate::agent::MsgIntegrity,
        broker_ts: i64,
    },
    Sent {
        conn: ConnId,
        msg_id: AgentMsgId,
    },
    Merr {
        conn: ConnId,
        msg_id: AgentMsgId,
        err: AgentFailure,
    },
    /// `RCVD`: the peer's receipt for a message we sent, and whether the hash it got matched.
    Rcvd {
        conn: ConnId,
        msg_id: AgentMsgId,
        status: crate::agent::client::RcptStatus,
    },
    /// A relay host connected (`UP`) or went away (`DOWN`).
    Up {
        server: String,
    },
    Down {
        server: String,
    },
    Err {
        conn: ConnId,
        err: AgentFailure,
    },
    /// A message on `conn` did not decrypt (`RSYNC` with `A_CRYPTO`); `count` is 1, or the
    /// messages skipped for `TooManySkipped`.
    DecryptError {
        conn: ConnId,
        error: crate::types::model::DecryptError,
        count: u32,
    },
    /// The links of a `create_address`: the full one and the short one when the relay stores it.
    Address {
        conn: ConnId,
        full: String,
        short: Option<String>,
    },
    /// `RSYNC`: the ratchet-sync state of `conn` changed (Agent/Protocol.hs `RatchetSyncState`).
    RatchetSync {
        conn: ConnId,
        state: crate::agent::resync::SyncState,
    },
    /// `SWITCH QDSnd`: the peer moves the queue we send to on `conn`.
    Switch {
        conn: ConnId,
        phase: crate::agent::client::SwitchPhase,
    },
    /// `LINK`: the user data `set_link_data` gave is stored.
    LinkUpdated {
        conn: ConnId,
    },
    /// `lookup_link` read and checked the link's data; `conn` is forgotten.
    LinkData {
        conn: ConnId,
        data: Box<crate::agent::connection::LinkLookup>,
    },
    /// `REQ`: someone used the address `conn`; `invitation` names the request to
    /// `accept_request`/`reject_request`, `conn_info` is their `x.contact`.
    Req {
        conn: ConnId,
        invitation: Vec<u8>,
        conn_info: Vec<u8>,
        pq: bool,
    },
}

/// The agent as the engine drives it: owned by the engine thread, never blocking. Every call
/// may queue events; the engine drains them after each call.
pub trait AgentPort {
    /// What SMP connection threads send to the engine thread, handed back through `on_smp`.
    type Smp: Send + 'static;

    /// `user_data` goes into the short link (the chat layer's `ContactShortLinkData`).
    fn create_invitation(
        &mut self,
        store: &mut Store,
        pq: bool,
        short: bool,
        user_data: Vec<u8>,
    ) -> Result<ConnId, AgentFailure>;
    fn join(
        &mut self,
        store: &mut Store,
        link: &ConnectionLink,
        conn_info: Vec<u8>,
        pq: bool,
    ) -> Result<ConnId, AgentFailure>;
    fn allow(
        &mut self,
        store: &mut Store,
        conn: &ConnId,
        conn_info: Vec<u8>,
    ) -> Result<(), AgentFailure>;
    /// `pq`: the connection's `pqEncryption`; on turns PQ support on for good (`sendMessage`).
    fn send(
        &mut self,
        store: &mut Store,
        conn: &ConnId,
        pq: bool,
        body: Vec<u8>,
    ) -> Result<AgentMsgId, AgentFailure>;
    /// `receipt`: `Some` asks for an `A_RCVD` (chat passes an empty one, `ackMessage … (Just "")`).
    fn ack(
        &mut self,
        store: &mut Store,
        conn: &ConnId,
        msg_id: AgentMsgId,
        receipt: Option<Vec<u8>>,
    ) -> Result<(), AgentFailure>;
    /// Deletes the connection once what was sent on it is delivered (`deleteConnectionsAsync`
    /// Deletes the connection; with `wait`, once what was sent on it is delivered
    /// (`deleteConnectionsAsync` with `waitDelivery`), as chat does after telling the peer
    /// (`x.direct.del`, `x.grp.leave`, …), so that still goes out.
    fn delete(&mut self, store: &mut Store, conn: &ConnId, wait: bool) -> Result<(), AgentFailure>;
    fn subscribe_all(&mut self, store: &mut Store) -> Result<(), AgentFailure>;
    fn on_smp(&mut self, store: &mut Store, event: Self::Smp);
    /// `now_ms`: the engine clock's wall time, the same clock `next_wake` is in.
    fn on_tick(&mut self, store: &mut Store, now_ms: i64);
    /// Wall-clock ms at which `on_tick` next has work; `None` when nothing waits.
    fn next_wake(&self) -> Option<i64>;
    /// Closes the network; what the agent stored stays.
    fn close(&mut self);
    fn drain_events(&mut self) -> Vec<AgentEvent>;

    /// A contact address (`createConnection … SCMContact`); answered by `AgentEvent::Address`.
    fn create_address(
        &mut self,
        store: &mut Store,
        short: bool,
        user_data: Vec<u8>,
    ) -> Result<ConnId, AgentFailure>;

    /// A group's link (`APICreateGroupLink`): a contact address whose `clientData` is
    /// `client_data` and whose short link is a `/g#` one; answered by `AgentEvent::Address`.
    fn create_group_address(
        &mut self,
        store: &mut Store,
        user_data: Vec<u8>,
        client_data: String,
    ) -> Result<ConnId, AgentFailure>;

    /// `getConnShortLink`: an address's link data, read without joining it; answered by
    /// `AgentEvent::LinkData`, or `AgentEvent::Err` on the returned connection.
    fn lookup_link(
        &mut self,
        store: &mut Store,
        link: &crate::agent::links::ConnShortLink,
    ) -> Result<ConnId, AgentFailure>;

    /// A channel's link (`prepareConnectionLink` + `createConnectionForLink`) under the
    /// channel's root key and id; `user_data` gets the short link. Answered by `Address`.
    fn create_channel_link(
        &mut self,
        store: &mut Store,
        root: &crate::crypto::PrivateKey,
        entity_id: Vec<u8>,
        owners: Vec<crate::agent::short_link::OwnerAuth>,
        user_data: LinkUserData,
        client_data: String,
    ) -> Result<ConnId, AgentFailure>;

    /// `setConnShortLink`: a channel link's owners, relays and user data; answered by `LinkUpdated`.
    fn set_link_data(
        &mut self,
        store: &mut Store,
        conn: &ConnId,
        owners: Vec<crate::agent::short_link::OwnerAuth>,
        relays: Vec<crate::agent::links::ConnShortLink>,
        user_data: Vec<u8>,
    ) -> Result<(), AgentFailure>;

    /// `acceptContact`: a new connection that answers the request, our `x.info` in `conn_info`.
    fn accept_request(
        &mut self,
        store: &mut Store,
        invitation: &[u8],
        conn_info: Vec<u8>,
        pq: bool,
    ) -> Result<ConnId, AgentFailure>;

    /// `rejectContact`: the request is forgotten; nothing is sent.
    fn reject_request(&mut self, store: &mut Store, invitation: &[u8]) -> Result<(), AgentFailure>;

    /// Whether `conn`'s ratchet uses the PQ KEM both ways, as negotiated (chat `connPQEnabled`).
    fn pq_on(&self, conn: &ConnId) -> bool;

    /// The ratchet's associated data of `conn` (`getConnectionRatchetAdHash` hashes it):
    /// what the security code is made from.
    fn ratchet_ad(&mut self, store: &mut Store, conn: &ConnId) -> Result<Vec<u8>, AgentFailure>;

    /// `synchronizeRatchet`: offers `conn`'s peer a new ratchet, allowed while its sync state
    /// is `Allowed` or `Required`, or at any time with `force`.
    fn synchronize_ratchet(
        &mut self,
        store: &mut Store,
        conn: &ConnId,
        force: bool,
    ) -> Result<(), AgentFailure>;

    /// What tests relays for the checks: the network, for the real agent. `None` where relays
    /// can't be tested (no threads); no check runs then, so nothing is judged offline by one.
    fn relay_tester(&self) -> Option<Box<dyn super::relays::RelayTester>>;

    /// Keeps new queues off the relays in `disabled` (addresses as the config gave them) while
    /// any other relay is configured; queues already on them stay. An empty list enables all.
    fn set_disabled_relays(&mut self, disabled: &[String]);

    /// Private message routing: when sends go through a relay of ours, and when they may go
    /// directly after all. Fakes that do not route ignore it.
    fn set_routing(&mut self, _mode: ProxyMode, _fallback: ProxyFallback) {}
}
