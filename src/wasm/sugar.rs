//! The named events over the engine's `Event`s: `event` for each, and `message`, `request`, `ready`, `offline`, `chats`.
use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::types::Event;
use crate::types::model::{ChatMessage, ChatProfile};

/// A message as JS gets it: the serde fields plus `mine` and `text`.
pub(super) fn message_value(m: &ChatMessage) -> Value {
    let mut v = serde_json::to_value(m).unwrap_or(Value::Null);
    if let Value::Object(o) = &mut v {
        o.insert("mine".into(), json!(m.mine()));
        o.insert("text".into(), json!(m.text()));
    }
    v
}

#[derive(Default)]
pub(super) struct Sugar {
    /// The requests the last chat list held, so each is announced once.
    /// Eviction: replaced by every chat list.
    requests: BTreeSet<i64>,
    /// The user's profile as the engine last said, for `setName`.
    pub(super) profile: Option<ChatProfile>,
}

impl Sugar {
    pub(super) fn calls(&mut self, e: &Event, out: &mut Vec<(&'static str, Value)>) {
        out.push(("event", serde_json::to_value(e).unwrap_or(Value::Null)));
        match e {
            Event::Messages(list) => {
                out.extend(list.iter().map(|m| ("message", message_value(m))));
            }
            Event::Chats {
                friends,
                groups,
                requests,
                chats,
            } => {
                let new = requests.iter().filter(|r| !self.requests.contains(&r.id));
                out.extend(new.map(|r| ("request", json!(r))));
                self.requests = requests.iter().map(|r| r.id).collect();
                let list = json!({ "friends": friends, "groups": groups, "requests": requests, "chats": chats });
                out.push(("chats", list));
            }
            Event::Ready(p) => {
                self.profile = Some(p.clone());
                out.push(("ready", json!(p)));
            }
            Event::Profile(p) => self.profile = Some(p.clone()),
            Event::Offline(offline) => out.push(("offline", json!(offline))),
            Event::Features { .. }
            | Event::Invite(_)
            | Event::Address(_)
            | Event::Updated(_)
            | Event::History { .. }
            | Event::Code { .. }
            | Event::Verified { .. }
            | Event::Members { .. }
            | Event::Preview { .. }
            | Event::OpenFile { .. }
            | Event::Located { .. }
            | Event::GroupLink { .. }
            | Event::Call(_)
            | Event::Stream(_)
            | Event::Failed { .. }
            | Event::Exited { .. } => {}
        }
    }
}
