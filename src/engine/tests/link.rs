//! Links with their previews, both ways (Protocol.hs `MCLink`, `LinkPreview`).

use crate::types::Request;
use crate::types::model::{ChatRef, LinkInfo, Picture};

use super::files::sent_events;
use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::content::MsgContent;

fn site() -> LinkInfo {
    LinkInfo {
        uri: "https://simplex.chat".into(),
        title: "SimpleX".into(),
        description: "private messaging".into(),
        image: Some(Picture {
            bytes: b"\x89PNG\r\n\x1a\nxx".to_vec(),
            mime: "image/png".into(),
        }),
    }
}

#[test]
fn a_link_goes_with_its_preview_and_shows_it() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let out = t.req(Request::SendLink {
        chat: ChatRef::Direct(bob),
        text: "see https://simplex.chat".into(),
        preview: site(),
        reply_to: None,
    });
    assert_eq!(messages(&out)[0].link, Some(site()));
    let Some(ChatEvent::MsgNew(c)) = sent_events(&t).pop() else {
        panic!()
    };
    let MsgContent::Link { text, preview } = c.content else {
        panic!("{:?}", c.content)
    };
    assert_eq!(text, "see https://simplex.chat");
    assert_eq!(preview.uri, "https://simplex.chat");
    assert!(preview.image.0.starts_with("data:image/png;base64,"));
}

#[test]
fn a_received_links_preview_is_kept_and_a_missing_picture_is_none() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    let mut preview = crate::engine::link::to_chat(&site());
    let without = LinkInfo {
        image: None,
        ..site()
    };
    let out = t.peer_says(
        &conn,
        1,
        ChatEvent::MsgNew(MsgContainer::simple(MsgContent::Link {
            text: "look".into(),
            preview: preview.clone(),
        })),
    );
    assert_eq!(
        messages(&out)[0].link.as_ref().unwrap().uri,
        "https://simplex.chat"
    );
    preview = crate::engine::link::to_chat(&without);
    let out = t.peer_says(
        &conn,
        2,
        ChatEvent::MsgNew(MsgContainer::simple(MsgContent::Link {
            text: "again".into(),
            preview,
        })),
    );
    assert_eq!(messages(&out)[0].link, Some(without));
}
