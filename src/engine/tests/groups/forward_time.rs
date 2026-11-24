//! A forward's time from a hostile admin: a huge year never overflows, in any build.

use super::super::{T, messages};
use super::forward::{from_host, fwd, json};
use super::messages::{member_id, one};
use crate::chat::batch::encode_binary_batch;
use crate::chat::container::MsgContainer;
use crate::chat::group::GroupMemberRole;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::clock::Clock;

#[test]
fn a_json_forward_with_a_huge_year_is_refused_without_overflowing() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let mut f = fwd(member_id(&t, g, ms[1].0), "carol");
    f.broker_ts = "9223372036854775807-01-01T00:00:00Z".into();
    let inner = ChatMessage::new(ChatEvent::MsgNew(MsgContainer::text("hi"))).unwrap();
    let event = ChatEvent::GrpMsgForward {
        fwd: f,
        msg: Box::new(inner),
    };
    let out = from_host(&mut t, &ms[0].1, 1, json(event));
    assert!(messages(&out).is_empty(), "{out:?}");
}

#[test]
fn a_binary_forward_with_the_largest_time_takes_the_time_it_arrived() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let id = member_id(&t, g, ms[1].0);
    let mut el = vec![b'>', b'M', id.as_bytes().len() as u8];
    el.extend_from_slice(id.as_bytes());
    el.extend_from_slice(b"\x05carol");
    el.extend_from_slice(&i64::MAX.to_be_bytes());
    el.extend_from_slice(&json(ChatEvent::MsgNew(MsgContainer::text("hi"))));
    let body = encode_binary_batch(&[el]).unwrap();
    let m = one(&from_host(&mut t, &ms[0].1, 1, body));
    assert_eq!(m.at, t.clock.wall_ms());
}
