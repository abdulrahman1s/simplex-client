//! The objects events carry act on the client: requests, friends, groups and messages.

use super::{builder, invitation, start, wait_for};
use crate::chat::profile::Profile;
use crate::chat::{ChatEvent, ChatMessage};
use crate::client::{Chat, Error, Event, Friend};
use crate::engine::agent_port::{AgentEvent, ConnId};
use crate::engine::tests::{Dir, FakeAgent, x_info};
use crate::types::model::Membership;
use crate::types::{Request, RequestKind};

const ADDRESS: &str = "simplex:/contact#/?v=6-8&smp=smp%3A%2F%2FAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE%3D%40fake.test%2F-FWjHkx7I4TOSuF3q4l2S4jD2BgprZRG%23%2F%3Fv%3D1-4%26dh%3DMCowBQYDK2VuAyEAQWIOFJ7ZTMqTFfTsHuyRYNDwszkpHQfRxObZXAWAIRU%253D%26q%3Dc";

fn contact(name: &str) -> Vec<u8> {
    let msg = ChatMessage::without_id(ChatEvent::Contact {
        profile: Profile::new(name, ""),
        member_key: None,
        contact_req_id: None,
        welcome_msg_id: None,
        request_msg: None,
    });
    msg.to_json().unwrap()
}

fn friend_added(client: &crate::client::Client) -> Friend {
    let Event::FriendAdd(f) = wait_for(client, |e| matches!(e, Event::FriendAdd(_))) else {
        unreachable!()
    };
    f
}

#[test]
fn a_request_object_accepts_and_the_requester_becomes_a_friend() {
    let dir = Dir::new();
    let (client, smp) = start(builder(&dir), FakeAgent::default());
    client.request(Request::CreateAddress).unwrap();
    let (address, friend) = (ConnId(1), ConnId(2));
    smp.send(AgentEvent::Address {
        conn: address,
        full: ADDRESS.into(),
        short: None,
    })
    .unwrap();
    let conn_info = contact("carol");
    let invitation = vec![5];
    smp.send(AgentEvent::Req {
        conn: address,
        invitation,
        conn_info,
        pq: true,
    })
    .unwrap();
    let Event::Request(req) = wait_for(&client, |e| matches!(e, Event::Request(_))) else {
        unreachable!()
    };
    assert_eq!(req.name(), "carol");
    req.accept().unwrap();
    let conn_info = x_info("carol");
    smp.send(AgentEvent::Info {
        conn: friend,
        conn_info,
    })
    .unwrap();
    smp.send(AgentEvent::Con { conn: friend }).unwrap();
    let carol = friend_added(&client);
    assert_eq!(carol.name(), "carol");
    assert!(client.friend(carol.id()).is_some_and(|f| f.ready()));
}

#[test]
fn a_new_friend_is_written_to_through_its_object() {
    let dir = Dir::new();
    let mut agent = FakeAgent::default();
    agent.on_invitation = vec![invitation()];
    let (client, smp) = start(builder(&dir), agent);
    client.create_invite().unwrap();
    let conn_info = x_info("bob");
    smp.send(AgentEvent::Conf {
        conn: ConnId(1),
        conn_info,
    })
    .unwrap();
    smp.send(AgentEvent::Con { conn: ConnId(1) }).unwrap();
    let bob = friend_added(&client);
    assert_eq!(client.friends().len(), 1);

    bob.send("welcome").unwrap();
    let Event::Message(m) = wait_for(&client, |e| matches!(e, Event::Message(m) if m.mine()))
    else {
        unreachable!()
    };
    assert_eq!(m.text(), "welcome");
    assert!(matches!(m.chat(), Chat::Friend(f) if f.id() == bob.id() && f.name() == "bob"));
}

#[test]
fn a_reply_goes_to_the_messages_chat_quoting_it() {
    let dir = Dir::new();
    let (client, _smp) = start(builder(&dir), FakeAgent::default());
    let club = client.create_group("Club").unwrap();
    assert_eq!(club.name(), "Club");
    club.send("hello").unwrap();
    let pick =
        |text: &'static str| move |e: &Event| matches!(e, Event::Message(m) if m.text() == text);
    let Event::Message(hello) = wait_for(&client, pick("hello")) else {
        unreachable!()
    };
    hello.reply("again").unwrap();
    let Event::Message(again) = wait_for(&client, pick("again")) else {
        unreachable!()
    };
    let quoted = again.info().reply.as_ref().and_then(|q| q.id);
    assert_eq!(quoted, Some(hello.id()));
    assert!(matches!(again.chat(), Chat::Group(g) if g.id() == club.id()));
}

#[test]
fn the_group_cache_follows_the_chat_list() {
    let dir = Dir::new();
    let (client, _smp) = start(builder(&dir), FakeAgent::default());
    let club = client.create_group("Club").unwrap();
    assert!(client.group(club.id()).is_some_and(|g| g.info().owner));
    club.leave().unwrap();
    let Event::GroupRemove(gone) = wait_for(&client, |e| matches!(e, Event::GroupRemove(_))) else {
        unreachable!()
    };
    assert_eq!(gone.id(), club.id());
    let left = client.group(club.id()).map(|g| g.info().membership);
    assert_eq!(left, Some(Membership::Left));
}

#[test]
fn a_refused_group_fails_its_create_call() {
    let dir = Dir::new();
    let (client, _smp) = start(builder(&dir), FakeAgent::default());
    let r = client.create_group("  ");
    assert!(
        matches!(
            r,
            Err(Error::Failed {
                request: RequestKind::Group,
                ..
            })
        ),
        "{r:?}"
    );
}
