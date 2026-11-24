//! A group member's invitation to talk directly: listed as a request where the group allows direct messages, and accepted by joining its link (Subscriber.hs `xGrpDirectInv`; Commands.hs `APIAcceptMemberContact`).

use crate::types::model::{ChatNotice, ChatRef, MessageBody};
use crate::types::{Event, Request};

use super::members_support::link;
use super::seed::profile;
use crate::chat::ChatEvent;
use crate::chat::content::MsgContent;
use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature, GroupPreferences};
use crate::engine::agent_port::ConnId;
use crate::engine::tests::{T, messages};

/// The user is a member of "team" with owner bob and carol; `direct` turns direct messages on.
pub fn team(t: &mut T, direct: bool) -> (i64, [(i64, ConnId); 2]) {
    let (g, m) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let mut rec = t.e.group(g).unwrap();
    rec.profile = profile("team");
    if direct {
        rec.profile.group_preferences = Some(GroupPreferences {
            direct_messages: Some(GroupFeature::new(FeatureEnabled::On)),
            ..GroupPreferences::default()
        });
    }
    t.e.save_group(rec).unwrap();
    (g, [m[0], m[1]])
}

pub fn invite(text: Option<&str>) -> ChatEvent {
    ChatEvent::GrpDirectInv {
        conn_req: link(3),
        content: text.map(|t| MsgContent::Text { text: t.into() }),
        scope: None,
    }
}

pub fn requests(t: &mut T) -> Vec<(i64, String)> {
    t.req(Request::Refresh)
        .into_iter()
        .find_map(|e| match e {
            Event::Chats { requests, .. } => {
                Some(requests.into_iter().map(|r| (r.id, r.name)).collect())
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn a_members_direct_invitation_is_a_request_where_the_group_allows_direct_messages() {
    let mut t = T::started();
    let (g, [_, (_, carol)]) = team(&mut t, true);
    let out = t.peer_says(&carol, 1, invite(None));
    let told = messages(&out).into_iter().map(|m| (m.chat, m.body));
    assert!(told.eq([(
        ChatRef::Group(g),
        MessageBody::Notice(ChatNotice::DirectInvitation)
    )]));
    t.peer_says(&carol, 2, invite(Some("again")));
    let [(_, name)] = requests(&mut t).try_into().unwrap();
    assert_eq!(name, "carol", "a second invitation replaces the first");
}

#[test]
fn a_members_direct_invitation_must_be_a_one_time_link() {
    let mut t = T::started();
    let (_, [_, (_, carol)]) = team(&mut t, true);
    let address = link(3).replace("/invitation", "/contact");
    assert!(crate::agent::links::ConnectionLink::parse(&address).is_ok());
    let event = ChatEvent::GrpDirectInv {
        conn_req: address,
        content: None,
        scope: None,
    };
    t.peer_says(&carol, 1, event);
    assert!(requests(&mut t).is_empty());
}

#[test]
fn a_members_direct_invitation_is_dropped_where_direct_messages_are_off() {
    let mut t = T::started();
    let (_, [_, (_, carol)]) = team(&mut t, false);
    t.peer_says(&carol, 1, invite(None));
    assert!(requests(&mut t).is_empty());
}

#[test]
fn a_blocked_members_direct_invitation_is_ignored() {
    let mut t = T::started();
    let (g, [_, (carol_id, carol)]) = team(&mut t, true);
    t.req(Request::SetMemberBlocked {
        group: g,
        member: carol_id,
        on: true,
    });
    t.peer_says(&carol, 1, invite(None));
    assert!(requests(&mut t).is_empty());
}

#[test]
fn deleting_the_address_keeps_a_members_invitation() {
    let mut t = T::started();
    let (_, [_, (_, carol)]) = team(&mut t, true);
    t.peer_says(&carol, 1, invite(None));
    t.req(Request::DeleteAddress);
    assert_eq!(requests(&mut t).len(), 1);
}

#[test]
fn a_request_record_from_before_member_invitations_reads_as_an_address_request() {
    let old = br#"{"v":1,"r":{"id":1,"invitation":[1],"name":"bob","about":"","image":null,"req_id":null,"since":0}}"#;
    let r: crate::engine::records::RequestRecord = crate::engine::records::decode(old).unwrap();
    assert_eq!(r.member, None);
}
