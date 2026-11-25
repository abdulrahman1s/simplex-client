//! Live messages, both ways (Commands.hs `APISendMessages` live, `APIUpdateChatItem`; Subscriber.hs `messageUpdate`; ChatTests/Direct.hs `testDirectLiveMessage`).

use crate::types::Request;
use crate::types::model::ChatRef;

use super::files::sent_events;
use super::timed::{later, set_timer};
use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::container::{MsgContainer, MsgMentions};
use crate::chat::content::MsgContent;
use crate::chat::ids::SharedMsgId;

#[test]
fn a_live_message_is_written_as_updates_and_its_timer_waits_until_it_is_done() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let chat = ChatRef::Direct(bob);
    set_timer(&mut t, bob, Some(5));
    let out = t.req(Request::SendLive {
        chat,
        text: "he".into(),
    });
    let m = messages(&out).pop().unwrap();
    assert!(m.live && m.timed.unwrap().delete_at.is_none());
    let Some(ChatEvent::MsgNew(c)) = sent_events(&t).pop() else {
        panic!()
    };
    assert_eq!(c.live, Some(true));
    t.req(Request::UpdateLive {
        chat,
        item: m.id,
        text: "hello".into(),
        done: false,
    });
    let Some(ChatEvent::MsgUpdate { live, .. }) = sent_events(&t).pop() else {
        panic!()
    };
    assert_eq!(live, Some(true));
    later(&mut t, 60);
    assert!(
        t.e.item(chat, m.id).unwrap().is_some(),
        "no count while live"
    );
    let out = t.req(Request::UpdateLive {
        chat,
        item: m.id,
        text: "hello there".into(),
        done: true,
    });
    let done = messages(&out).pop().unwrap();
    assert!(!done.live && !done.edited && done.timed.unwrap().delete_at.is_some());
    assert!(matches!(
        sent_events(&t).pop(),
        Some(ChatEvent::MsgUpdate { live: None, .. })
    ));
    later(&mut t, 6);
    assert!(t.e.item(chat, m.id).unwrap().is_none());
}

#[test]
fn a_friends_live_message_updates_without_being_edited() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    let mut c = MsgContainer::text("ty");
    c.live = Some(true);
    let msg = crate::chat::ChatMessage::new(ChatEvent::MsgNew(c)).unwrap();
    let id = msg.msg_id.clone().unwrap();
    let body = crate::chat::encode_chat_message(&msg, false).unwrap();
    let got = t.agent(crate::engine::agent_port::AgentEvent::Msg {
        conn,
        msg_id: 1,
        body,
        integrity: crate::agent::MsgIntegrity::Ok,
        broker_ts: 0,
    });
    assert!(messages(&got)[0].live);
    let update = |text: &str, live| ChatEvent::MsgUpdate {
        msg_id: SharedMsgId(id.0.clone()),
        content: MsgContent::Text { text: text.into() },
        mentions: MsgMentions::new(),
        ttl: None,
        live,
        scope: None,
        as_group: None,
    };
    let m = messages(&t.peer_says(&conn, 2, update("typing", Some(true))))[0].clone();
    assert!(m.live && !m.edited && m.text() == "typing");
    let m = messages(&t.peer_says(&conn, 3, update("typing done", None)))[0].clone();
    assert!(!m.live && !m.edited && m.text() == "typing done");
    let m = messages(&t.peer_says(&conn, 4, update("changed", None)))[0].clone();
    assert!(m.edited, "an edit after it is done is one");
}
