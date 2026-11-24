//! A new member gets the group's history from its host when the group has it on (Internal.hs `sendHistory`; Store/Messages.hs `getGroupHistoryItems`).

use std::time::Duration;

use crate::types::model::{ChatRef, MessageAuthor, MessageBody};
use crate::types::{FileKind, Request};

use super::super::files::{known, parts, picture, put, setup};
use super::super::{T, messages};
use super::members::acpt;
use super::members_support::link;
use super::messages::{member_id, says, send};
use crate::chat::container::MsgContainer;
use crate::chat::forward::FwdSender;
use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature, GroupPreferences};
use crate::chat::ids::MemberId;
use crate::chat::{ChatEvent, parse_chat_elements};
use crate::engine::agent_port::{AgentEvent, ConnId};
use crate::engine::files::transfers::SentToMembers;
use crate::engine::files::{FileDone, FileKey};
use crate::engine::jobs::JobDone;
use crate::engine::time::iso;

/// alice owns "team" with carol, history on or off as `on` says.
pub fn team(t: &mut T, on: bool) -> (i64, ConnId) {
    let (g, ms) = t.group("team", &["carol"], GroupMemberRole::Owner);
    let mut gr = t.e.state.groups[&g].clone();
    let enable = if on {
        FeatureEnabled::On
    } else {
        FeatureEnabled::Off
    };
    gr.profile.group_preferences = Some(GroupPreferences {
        history: Some(GroupFeature::new(enable)),
        ..GroupPreferences::default()
    });
    t.e.save_group(gr).unwrap();
    (g, ms[0].1)
}

/// Friend dave is added to `g` and his CONF is in: his member connection.
pub fn dave_accepted(t: &mut T, g: i64) -> ConnId {
    let (dave, _) = t.friend("dave");
    t.req(Request::AddMember {
        group: g,
        friend: dave,
    });
    let m =
        t.e.state
            .members_of(g)
            .find(|m| m.friend == Some(dave))
            .unwrap()
            .clone();
    let conn = m.conn.unwrap();
    t.agent(AgentEvent::Invitation {
        conn,
        full: link(1),
        short: None,
    });
    let info = acpt(t, g, m.id, "dave");
    t.agent(AgentEvent::Conf {
        conn,
        conn_info: info,
    });
    conn
}

/// The forwards sent on `conn`: (author, time, event); `None` for a plain element.
pub fn history(t: &T, conn: ConnId) -> Vec<(Option<(MemberId, String)>, ChatEvent)> {
    t.e.agent
        .sent()
        .into_iter()
        .filter(|(c, _)| *c == conn)
        .flat_map(|(_, b)| parse_chat_elements(&b))
        .map(|p| {
            let p = p.unwrap();
            let from = p.forward.map(|f| match f.sender {
                FwdSender::Member { member_id, .. } => (member_id, f.broker_ts),
                other => panic!("{other:?}"),
            });
            (from, p.message.event)
        })
        .filter(|(_, e)| !matches!(e, ChatEvent::GrpMemIntro { .. }))
        .collect()
}

fn text(e: &ChatEvent) -> &str {
    match e {
        ChatEvent::MsgNew(c) => c.content.text(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_new_member_gets_the_groups_history_with_each_items_author_and_time_then_the_welcome() {
    let mut t = T::started();
    let (g, carol_conn) = team(&mut t, true);
    let mut gr = t.e.state.groups[&g].clone();
    gr.profile.description = Some("welcome!".into());
    t.e.save_group(gr).unwrap();
    // The binary forward header has whole seconds.
    let at = |t: &T| iso(t.e.clock.wall_ms()).replace(".000Z", "Z");
    let carol_at = at(&t);
    says(&mut t, &carol_conn, 1, MsgContainer::text("from carol"));
    t.clock.advance(Duration::from_secs(60));
    let alice_at = at(&t);
    t.req(send(g, "from alice", vec![]));
    let conn = dave_accepted(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    let got = history(&t, conn);
    let carol =
        t.e.state
            .members_of(g)
            .find(|m| m.profile.display_name == "carol")
            .unwrap();
    let alice = t.e.state.groups[&g].my_member_id.clone();
    let [(Some(c), c_msg), (Some(a), a_msg), (None, welcome)] = got.as_slice() else {
        panic!("{got:?}")
    };
    assert_eq!(
        (c, text(c_msg)),
        (&(carol.member_id.clone(), carol_at), "from carol")
    );
    assert_eq!((a, text(a_msg)), (&(alice, alice_at), "from alice"));
    assert_eq!(text(welcome), "welcome!");
}

#[test]
fn without_history_a_new_member_gets_only_its_introductions() {
    let mut t = T::started();
    let (g, carol_conn) = team(&mut t, false);
    says(&mut t, &carol_conn, 1, MsgContainer::text("from carol"));
    let conn = dave_accepted(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    assert!(history(&t, conn).is_empty());
}

#[test]
fn a_message_that_waited_for_the_member_is_not_sent_again_as_history() {
    let mut t = T::started();
    let (g, _) = team(&mut t, true);
    t.req(send(g, "before", vec![]));
    let conn = dave_accepted(&mut t, g);
    t.req(send(g, "while dave connects", vec![]));
    t.agent(AgentEvent::Con { conn });
    let got: Vec<String> = history(&t, conn)
        .iter()
        .map(|(_, e)| text(e).to_owned())
        .collect();
    assert_eq!(got, ["while dave connects", "before"]);
}

#[test]
fn history_holds_the_newest_100_messages_and_no_notices() {
    let mut t = T::started();
    let (g, _) = team(&mut t, true);
    for n in 0..105 {
        t.req(send(g, &format!("m{n}"), vec![]));
    }
    let conn = dave_accepted(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    let got: Vec<String> = history(&t, conn)
        .iter()
        .map(|(_, e)| text(e).to_owned())
        .collect();
    assert_eq!(got.len(), 100);
    assert_eq!((got[0].as_str(), got[99].as_str()), ("m5", "m104"));
}

#[test]
fn a_members_file_in_history_comes_with_its_whole_description() {
    let (mut t, fake) = setup();
    fake.with(|s| s.save_as = "p.jpg".into());
    let (g, carol_conn) = team(&mut t, true);
    let m = messages(&t.peer_says(&carol_conn, 1, picture("p.jpg", 2048))).remove(0);
    let shared =
        t.e.item(ChatRef::Group(g), m.id)
            .unwrap()
            .unwrap()
            .shared_id
            .unwrap();
    for (n, p) in parts(&shared, &known(), 9000).into_iter().enumerate() {
        t.peer_says(&carol_conn, 10 + n as i64, p);
    }
    let conn = dave_accepted(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    let got = history(&t, conn);
    let ChatEvent::MsgNew(c) = &got[0].1 else {
        panic!("{got:?}")
    };
    assert_eq!(c.file.as_ref().unwrap().file_name, "p.jpg");
    assert_eq!(c.content.text(), "look");
    let descr: String = got[1..]
        .iter()
        .map(|(_, e)| match e {
            ChatEvent::MsgFileDescr { file_descr, .. } => file_descr.file_descr_text.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(descr, known());
    assert!(
        got.iter()
            .all(|(from, _)| from.as_ref().unwrap().0 == member_id(&t, g, 1))
    );
}

#[test]
fn the_users_file_in_history_comes_with_the_description_a_member_got() {
    let (mut t, fake) = setup();
    fake.with(|s| s.hold = true);
    let (g, _) = team(&mut t, true);
    let path = put(&t, "clip.pdf");
    let out = t.req(Request::SendFile {
        chat: ChatRef::Group(g),
        path,
        kind: FileKind::File,
        preview: None,
        duration: None,
        text: String::new(),
    });
    let key = FileKey::of(ChatRef::Group(g), messages(&out)[0].id);
    let job = fake.with(|s| s.jobs[0]);
    let result = Ok(SentToMembers {
        descriptions: vec!["for carol".into()],
        expires: None,
    });
    t.e.on_job(JobDone::File(FileDone::UploadedForMembers {
        key,
        job,
        result,
    }));
    let conn = dave_accepted(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    let got = history(&t, conn);
    let [
        (_, ChatEvent::MsgNew(c)),
        (_, ChatEvent::MsgFileDescr { file_descr, .. }),
    ] = got.as_slice()
    else {
        panic!("{got:?}")
    };
    assert_eq!(c.file.as_ref().unwrap().file_name, "clip.pdf");
    assert_eq!(file_descr.file_descr_text, "for carol");
}

#[test]
fn a_joiner_engine_shows_the_history_it_is_sent_with_its_authors() {
    let mut t = T::started();
    let (g, carol_conn) = team(&mut t, true);
    says(&mut t, &carol_conn, 1, MsgContainer::text("from carol"));
    t.req(send(g, "from alice", vec![]));
    let conn = dave_accepted(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    let bodies: Vec<Vec<u8>> =
        t.e.agent
            .sent()
            .into_iter()
            .filter(|(c, _)| *c == conn)
            .map(|(_, b)| b)
            .collect();
    // A host's group as dave's engine holds it, with alice and carol known.
    let mut joiner = T::started();
    let (jg, jms) = joiner.group("team", &["alice", "carol"], GroupMemberRole::Member);
    for (m, id) in [
        (jms[0].0, t.e.state.groups[&g].my_member_id.clone()),
        (jms[1].0, member_id(&t, g, 1)),
    ] {
        let mut r = joiner.e.state.members[&(jg, m)].clone();
        r.member_id = id;
        joiner.e.save_member(r).unwrap();
    }
    let mut shown = Vec::new();
    for (n, body) in bodies.into_iter().enumerate() {
        let out = joiner.agent(AgentEvent::Msg {
            conn: jms[0].1,
            msg_id: 1 + n as i64,
            body,
        });
        shown.extend(messages(&out).into_iter().map(|m| (m.author, m.body)));
    }
    assert_eq!(
        shown,
        [
            (
                MessageAuthor::Member("carol".into()),
                MessageBody::Text("from carol".into())
            ),
            (
                MessageAuthor::Member("alice".into()),
                MessageBody::Text("from alice".into())
            ),
        ]
    );
}

#[test]
fn a_group_made_here_sends_its_history_to_a_member_it_adds() {
    let mut t = T::started();
    t.req(Request::NewGroup("team".into()));
    let g = *t.e.state.groups.keys().next().unwrap();
    t.req(send(g, "first", vec![]));
    let conn = dave_accepted(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    let got: Vec<String> = history(&t, conn)
        .iter()
        .map(|(_, e)| text(e).to_owned())
        .collect();
    assert_eq!(got, ["first"]);
}
