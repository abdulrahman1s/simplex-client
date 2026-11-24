//! Forward: a message goes to another chat as the user's own, marked forwarded (Commands.hs `APIForwardChatItems`; ChatTests/Forward.hs).

use crate::types::model::ChatRef;
use crate::types::{FileKind, Request};

use super::files::{put, sent_events, setup};
use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::engine::agent_port::ConnId;

fn new_to(t: &T, conn: &ConnId) -> Vec<MsgContainer> {
    let sent = t.e.agent.sent();
    let to = sent.iter().filter(|(c, _)| c == conn);
    to.flat_map(|(_, b)| crate::chat::parse_chat_messages(b).unwrap())
        .filter_map(|m| match m.event {
            ChatEvent::MsgNew(c) => Some(c),
            _ => None,
        })
        .collect()
}

#[test]
fn a_friends_message_goes_to_another_as_the_users_own_marked_forwarded() {
    let mut t = T::started();
    let (bob, bob_conn) = t.friend("bob");
    let (carol, carol_conn) = t.friend("carol");
    let got = t.peer_says(&bob_conn, 1, ChatEvent::MsgNew(MsgContainer::text("news")));
    let item = messages(&got)[0].id;
    let out = t.req(Request::Forward {
        from: ChatRef::Direct(bob),
        item,
        to: ChatRef::Direct(carol),
    });
    let m = &messages(&out)[0];
    assert!(m.forwarded && m.text() == "news");
    let [c] = new_to(&t, &carol_conn).try_into().unwrap();
    assert_eq!((c.forward, c.content.text()), (Some(true), "news"));
}

#[test]
fn a_file_here_goes_again_and_one_not_here_sends_its_caption() {
    let (mut t, _) = setup();
    let (bob, _) = t.friend("bob");
    let (carol, carol_conn) = t.friend("carol");
    let path = put(&t, "doc.pdf");
    let sent = t.req(Request::SendFile {
        chat: ChatRef::Direct(bob),
        path,
        kind: FileKind::File,
        preview: None,
        duration: None,
        text: "the doc".into(),
    });
    let item = messages(&sent)[0].id;
    t.req(Request::Forward {
        from: ChatRef::Direct(bob),
        item,
        to: ChatRef::Direct(carol),
    });
    let [c] = new_to(&t, &carol_conn).try_into().unwrap();
    assert!(c.file.is_some() && c.forward == Some(true));
    assert_eq!(c.content.text(), "the doc");
    let _ = sent_events(&t);
}

#[test]
fn a_received_forward_shows_as_one_and_a_notice_is_not_forwarded() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let mut c = MsgContainer::text("passed on");
    c.forward = Some(true);
    let got = t.peer_says(&conn, 1, ChatEvent::MsgNew(c));
    assert!(messages(&got)[0].forwarded);
    let notice = t.e.items_page(ChatRef::Direct(bob), None, 10).unwrap()[0].id;
    let out = t.req(Request::Forward {
        from: ChatRef::Direct(bob),
        item: notice,
        to: ChatRef::Direct(bob),
    });
    assert!(
        out.iter()
            .any(|e| matches!(e, crate::types::Event::Failed { .. }))
    );
}
