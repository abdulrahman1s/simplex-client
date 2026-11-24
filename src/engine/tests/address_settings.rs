//! The address's settings and showing it in the profile (Commands.hs `APISetAddressSettings`, `APISetProfileAddress`; ChatTests/Profiles.hs `testAutoReplyMessage`).

use crate::types::model::{ChatRef, MessageAuthor, MessageBody};
use crate::types::{Event, Request};

use super::address::{asks, contact, made};
use super::files::sent_events;
use super::{Call, T, messages, x_info};
use crate::chat::ChatEvent;
use crate::engine::agent_port::AgentEvent;

#[test]
fn a_request_is_accepted_as_it_comes_and_the_auto_reply_goes_once_connected() {
    let mut t = T::started();
    let address = made(&mut t);
    let out = t.req(Request::SetAddressSettings {
        auto_accept: true,
        auto_reply: Some(" Hi! I'm a bot. ".into()),
    });
    let Some(Event::Profile(p)) = out.iter().find(|e| matches!(e, Event::Profile(_))) else {
        panic!("{out:?}")
    };
    assert!(p.auto_accept && p.auto_reply.as_deref() == Some("Hi! I'm a bot."));
    asks(&mut t, address, 1, contact("dora", Some(b"r1")));
    let accepted =
        t.e.agent
            .calls
            .iter()
            .any(|c| matches!(c, Call::AcceptRequest { .. }));
    assert!(accepted, "accepted without asking the user");
    assert!(t.e.state.address.requests.is_empty());
    let (&dora, f) = t.e.state.friends.iter().next().unwrap();
    let conn = f.conn;
    assert!(
        sent_events(&t)
            .iter()
            .all(|e| !matches!(e, ChatEvent::MsgNew(_))),
        "not before connecting"
    );
    t.agent(AgentEvent::Conf {
        conn,
        conn_info: x_info("dora"),
    });
    let out = t.agent(AgentEvent::Con { conn });
    let mine = messages(&out)
        .into_iter()
        .find(|m| m.author == MessageAuthor::Me)
        .unwrap();
    assert_eq!(mine.body, MessageBody::Text("Hi! I'm a bot.".into()));
    assert_eq!(mine.chat, ChatRef::Direct(dora));
    let n = sent_events(&t)
        .iter()
        .filter(|e| matches!(e, ChatEvent::MsgNew(_)))
        .count();
    t.agent(AgentEvent::Con { conn });
    let again = sent_events(&t)
        .iter()
        .filter(|e| matches!(e, ChatEvent::MsgNew(_)))
        .count();
    assert_eq!(n, again, "once");
}

#[test]
fn the_address_shows_in_the_profile_until_it_is_taken_out_or_deleted() {
    let mut t = T::started();
    let (_, bob_conn) = t.friend("bob");
    assert!(
        t.req(Request::ShareAddress(true))
            .iter()
            .any(|e| matches!(e, Event::Failed { .. })),
        "no address yet"
    );
    made(&mut t);
    t.req(Request::ShareAddress(true));
    let link = |t: &T| {
        let sent = t.e.agent.sent();
        let to_bob = sent.iter().filter(|(c, _)| *c == bob_conn);
        let infos = to_bob.flat_map(|(_, b)| crate::chat::parse_chat_messages(b).unwrap());
        infos
            .filter_map(|m| match m.event {
                ChatEvent::Info { profile, .. } => Some(profile.contact_link),
                _ => None,
            })
            .next_back()
            .flatten()
    };
    assert!(link(&t).is_some_and(|l| l.starts_with("simplex:/contact")));
    t.req(Request::DeleteAddress);
    assert_eq!(link(&t), None, "a deleted address leaves the profile");
}

#[test]
fn a_request_the_user_accepts_gets_the_auto_reply_too() {
    let mut t = T::started();
    let address = made(&mut t);
    t.req(Request::SetAddressSettings {
        auto_accept: false,
        auto_reply: Some("welcome".into()),
    });
    asks(&mut t, address, 1, contact("dora", Some(b"r1")));
    let request = *t.e.state.address.requests.keys().next().unwrap();
    t.req(Request::Accept(request));
    let conn = t.e.state.friends.values().next().unwrap().conn;
    t.agent(AgentEvent::Conf {
        conn,
        conn_info: x_info("dora"),
    });
    let out = t.agent(AgentEvent::Con { conn });
    assert!(
        messages(&out)
            .iter()
            .any(|m| m.body == MessageBody::Text("welcome".into()))
    );
}
