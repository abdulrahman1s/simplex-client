//! The engine's events as JS hears them: `raw` for each, then what it means (`message`, `friendAdd`, …).
use serde_json::Value;

use super::outcome::Outcome;
use super::payload::Payload;
use super::runtime::Core;
use crate::roster::{Note, lift};
use crate::types::Event;

impl Core {
    pub(super) fn hear(&mut self, e: &Event, out: &mut Outcome) {
        let raw = serde_json::to_value(e).unwrap_or(Value::Null);
        out.calls.push(("raw", Payload::Json(raw)));
        if let Event::Ready(p) | Event::Profile(p) = e {
            self.profile = Some(p.clone());
        }
        for note in lift(e, &mut self.roster) {
            if let Note::GroupAdd(g) = &note
                && g.owner
            {
                out.settled.extend(self.waiters.created(g));
            }
            out.calls.push(named(note));
        }
    }
}

fn named(note: Note) -> (&'static str, Payload) {
    match note {
        Note::Ready(p) => (
            "ready",
            Payload::Json(serde_json::to_value(p).unwrap_or(Value::Null)),
        ),
        Note::Message(m) => ("message", Payload::Message(Box::new(m))),
        Note::MessageUpdate(m) => ("messageUpdate", Payload::Message(Box::new(m))),
        Note::Request(r) => ("request", Payload::Request(r)),
        Note::FriendAdd(f) => ("friendAdd", Payload::Friend(Box::new(f))),
        Note::FriendRemove(f) => ("friendRemove", Payload::Friend(Box::new(f))),
        Note::FriendUpdate(f) => ("friendUpdate", Payload::Friend(Box::new(f))),
        Note::GroupAdd(g) => ("groupAdd", Payload::Group(Box::new(g))),
        Note::GroupRemove(g) => ("groupRemove", Payload::Group(Box::new(g))),
        Note::GroupUpdate(g) => ("groupUpdate", Payload::Group(Box::new(g))),
        Note::GroupInvite(g) => ("groupInvite", Payload::Invite(Box::new(g))),
        Note::MemberAdd { group, name } => ("memberAdd", Payload::Member { group, name }),
        Note::Offline(true) => ("offline", Payload::Nothing),
        Note::Offline(false) => ("online", Payload::Nothing),
        Note::SendStatus {
            tag,
            member,
            status,
        } => {
            let v = serde_json::json!({"tag": tag, "member": member, "status": status});
            ("sendStatus", Payload::Json(v))
        }
        Note::Data {
            chat,
            member,
            tag,
            json,
        } => (
            "data",
            Payload::Data {
                chat,
                member,
                tag,
                json: Value::Object(json),
            },
        ),
    }
}
