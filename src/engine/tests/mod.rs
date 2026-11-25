//! Engine rule tests against `FakeAgent` and `FakeClock`, named after the codec tests they port.

mod address;
mod address_join;
mod address_settings;
mod calls;
mod change;
mod codes;
mod conn_events;
mod contact;
pub(crate) mod data;
mod decrypt_error;
mod fake_agent;
mod files;
mod forward;
mod friend_pq;
pub(crate) mod groups;
mod handshake;
mod inbound;
mod incognito;
mod integrity;
mod limits;
mod link;
mod live_address;
mod live_autoreply;
mod live_call;
mod live_cli;
mod live_data;
mod live_forward;
mod live_group;
mod live_incognito;
mod live_link;
mod live_livemsg;
mod live_manage;
mod live_member_contact;
mod live_msg;
mod live_offline;
mod live_prefs;
mod live_repair;
mod live_timed;
mod live_voice;
mod local;
mod local_chat;
mod msg_delete;
mod prefs;
mod prefs_set;
mod profile;
mod relays;
mod relays_fake;
mod relays_offline;
mod relays_pass;
mod relays_presets;
mod relays_retry;
mod restart;
mod resync;
mod send;
mod start;
mod streams;
pub(crate) mod timed;
mod timed_keep;
mod timed_prefs;

use std::path::PathBuf;

use crate::types::model::ChatMessage;
use crate::types::{Config, Event, Request};

use super::agent_port::{AgentEvent, ConnId};
use super::clock::FakeClock;
use super::machine::Engine;
use crate::chat::profile::Profile;
use crate::chat::{ChatEvent, ChatMessage as XMessage, encode_chat_message};
use crate::store::Store;
pub use fake_agent::{Call, FakeAgent};

pub const KEY: &str = "test key";

/// A scratch directory removed on drop.
pub struct Dir(pub PathBuf);

impl Dir {
    pub fn new() -> Dir {
        let mut id = [0u8; 8];
        getrandom::fill(&mut id).unwrap();
        let name: String = id.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("simplex-engine-{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }

    pub fn hello(&self, name: &str) -> Config {
        Config {
            dir: self.0.clone(),
            key: KEY.into(),
            name: name.into(),
            relays: Vec::new(),
            proxy_mode: crate::types::ProxyMode::default(),
            proxy_fallback: crate::types::ProxyFallback::default(),
            data_tags: std::collections::BTreeSet::new(),
        }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct T {
    pub e: Engine<FakeAgent>,
    pub clock: FakeClock,
    pub dir: Dir,
    pub relays: relays_pass::Passing,
}

impl T {
    /// An engine for "alice", not started.
    pub fn new() -> T {
        let dir = Dir::new();
        let store = Store::open(&dir.0.join("chat.db"), KEY).unwrap();
        let clock = FakeClock::new(1_795_000_000_000);
        let (e, relays) = relays_pass::engine(store, &clock);
        T {
            e,
            clock,
            dir,
            relays,
        }
    }

    /// Started, with what the start put out taken.
    pub fn started() -> T {
        let mut t = T::new();
        t.e.start(&t.dir.hello("alice")).unwrap();
        t.out();
        t
    }

    /// The same store with a new engine, started, a minute later; what the start put out taken.
    pub fn restarted(self) -> T {
        let T { e, clock, dir, .. } = self;
        drop(e);
        clock.advance(std::time::Duration::from_secs(60));
        let store = Store::open(&dir.0.join("chat.db"), KEY).unwrap();
        let (mut e, relays) = relays_pass::engine(store, &clock);
        e.start(&dir.hello("alice")).unwrap();
        let mut t = T {
            e,
            clock,
            dir,
            relays,
        };
        t.out();
        t
    }

    /// What was put out, after the relay checks' answers are handed in.
    pub fn out(&mut self) -> Vec<Event> {
        loop {
            let done = self.relays.take();
            if done.is_empty() {
                break;
            }
            for d in done {
                self.e.on_job(super::jobs::JobDone::Relay(d));
            }
        }
        std::iter::from_fn(|| self.e.pop_out()).collect()
    }

    pub fn req(&mut self, r: Request) -> Vec<Event> {
        self.e.on_request(r);
        self.out()
    }

    pub fn agent(&mut self, ev: AgentEvent) -> Vec<Event> {
        self.e.agent.queued.push(ev);
        self.e.pump();
        self.out()
    }

    /// Someone named `name` answers a fresh invitation and connects: their friend id and conn.
    pub fn friend(&mut self, name: &str) -> (i64, ConnId) {
        self.req(Request::CreateInvite);
        let conn = self.e.state.inviting.unwrap();
        self.agent(AgentEvent::Invitation {
            conn,
            full: format!("simplex:/invitation#/?v=2-7&name={name}"),
            short: None,
        });
        self.agent(AgentEvent::Conf {
            conn,
            conn_info: x_info(name),
        });
        self.agent(AgentEvent::Con { conn });
        (self.e.state.friend_of(&conn).unwrap(), conn)
    }

    /// A message from the peer on `conn`, numbered `msg_id` by the agent.
    pub fn peer_says(&mut self, conn: &ConnId, msg_id: i64, event: ChatEvent) -> Vec<Event> {
        let msg = XMessage::new(event).unwrap();
        let body = encode_chat_message(&msg, false).unwrap();
        self.agent(AgentEvent::Msg {
            conn: *conn,
            msg_id,
            body,
            integrity: crate::agent::MsgIntegrity::Ok,
            broker_ts: crate::engine::clock::Clock::wall_ms(&self.clock) / 1000,
        })
    }

    pub fn acks(&self) -> Vec<(i64, Option<Vec<u8>>)> {
        self.e
            .agent
            .calls
            .iter()
            .filter_map(|c| {
                if let Call::Ack {
                    msg_id, receipt, ..
                } = c
                {
                    Some((*msg_id, receipt.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

pub fn x_info(name: &str) -> Vec<u8> {
    let msg = XMessage::without_id(ChatEvent::Info {
        profile: Profile::new(name, ""),
        member_key: None,
    });
    msg.to_json().unwrap()
}

/// The messages of the only `Messages` or `Updated` event.
pub fn messages(out: &[Event]) -> Vec<ChatMessage> {
    out.iter()
        .flat_map(|e| {
            if let Event::Messages(m) | Event::Updated(m) = e {
                m.clone()
            } else {
                Vec::new()
            }
        })
        .collect()
}
