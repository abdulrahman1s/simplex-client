//! A tagged file send: one word per recipient, after its description, or a failure.

use crate::types::model::{ChatRef, SendStatus};
use crate::types::{FileKind, Request};

use super::super::files::{known, put, settle, setup};
use super::super::{T, messages};
use super::{statuses, tagged};
use crate::chat::group::GroupMemberRole;
use crate::engine::agent_port::{AgentEvent, AgentFailure, AgentMsgId, ConnId};
use crate::engine::files::transfers::{Sent, SentToMembers};
use crate::engine::files::{FileDone, FileKey};
use crate::engine::jobs::JobDone;

fn file(t: &T, chat: ChatRef) -> Request {
    Request::SendFile {
        chat,
        path: put(t, "a.bin"),
        kind: FileKind::File,
        preview: None,
        duration: None,
        text: String::new(),
    }
}

/// The tagged messages waiting, oldest first per connection.
fn waiting(t: &T) -> Vec<(ConnId, AgentMsgId)> {
    t.e.send_tags.keys().copied().collect()
}

fn sent(t: &mut T, (conn, msg_id): (ConnId, AgentMsgId)) -> Vec<(u64, Option<i64>, SendStatus)> {
    statuses(&t.agent(AgentEvent::Sent { conn, msg_id }))
}

fn failed(detail: &str) -> SendStatus {
    SendStatus::Failed {
        detail: detail.into(),
    }
}

#[test]
fn a_tagged_file_is_told_sent_once_its_description_went_not_with_its_offer() {
    let (mut t, fake) = setup();
    fake.with(|s| {
        s.description = known();
        s.hold = true;
    });
    let (bob, _) = t.friend("bob");
    let out = t.req(tagged(20, file(&t, ChatRef::Direct(bob))));
    let item = messages(&out)[0].id;
    let queued = SendStatus::Queued { item: Some(item) };
    assert_eq!(statuses(&out), [(20, None, queued)]);
    let [offer] = waiting(&t)[..] else { panic!() };
    assert!(
        sent(&mut t, offer).is_empty(),
        "the offer alone is not the file"
    );
    fake.with(|s| s.hold = false);
    let job = fake.with(|s| s.jobs[0]);
    let key = FileKey::of(ChatRef::Direct(bob), item);
    let result = Ok(Sent {
        description: known(),
        expires: None,
    });
    use crate::engine::files::FileDone;
    t.e.on_job(JobDone::File(FileDone::Uploaded { key, job, result }));
    let [descr] = waiting(&t)[..] else { panic!() };
    assert_eq!(sent(&mut t, descr), [(20, None, SendStatus::Sent)]);
    assert!(t.e.files.tags.is_empty() && t.e.send_tags.is_empty());
}

#[test]
fn a_tagged_file_whose_upload_fails_is_told_failed_once() {
    let (mut t, fake) = setup();
    fake.with(|s| s.fail = true);
    let (bob, _) = t.friend("bob");
    t.req(tagged(21, file(&t, ChatRef::Direct(bob))));
    let offer = waiting(&t)[0];
    let told = statuses(&settle(&mut t));
    assert_eq!(told, [(21, None, failed("upload failed: no server"))]);
    assert!(sent(&mut t, offer).is_empty());
    assert!(t.e.files.tags.is_empty() && t.e.send_tags.is_empty());
}

#[test]
fn a_cancelled_tagged_file_is_told_failed() {
    let (mut t, fake) = setup();
    fake.with(|s| s.hold = true);
    let (bob, _) = t.friend("bob");
    let out = t.req(tagged(22, file(&t, ChatRef::Direct(bob))));
    let item = messages(&out)[0].id;
    let chat = ChatRef::Direct(bob);
    let out = t.req(Request::CancelFile { chat, item });
    assert_eq!(
        statuses(&out),
        [(22, None, failed("the file was cancelled"))]
    );
    assert!(t.e.files.tags.is_empty());
}

#[test]
fn a_tagged_file_deleted_while_it_uploads_is_told_failed() {
    let (mut t, fake) = setup();
    fake.with(|s| s.hold = true);
    let (bob, _) = t.friend("bob");
    let out = t.req(tagged(25, file(&t, ChatRef::Direct(bob))));
    let item = messages(&out)[0].id;
    let chat = ChatRef::Direct(bob);
    t.req(Request::Delete { chat, item });
    let key = FileKey::of(chat, item);
    let job = fake.with(|s| s.jobs[0]);
    let result = Ok(Sent {
        description: known(),
        expires: None,
    });
    t.e.on_job(JobDone::File(FileDone::Uploaded { key, job, result }));
    assert_eq!(
        statuses(&t.out()),
        [(25, None, failed("the file was deleted"))]
    );
}

#[test]
fn a_description_in_parts_is_told_sent_with_its_last_and_failed_with_any() {
    let (mut t, fake) = setup();
    fake.with(|s| s.description = "x".repeat(30_000));
    let (bob, _) = t.friend("bob");
    t.req(tagged(23, file(&t, ChatRef::Direct(bob))));
    settle(&mut t);
    let w = waiting(&t);
    assert_eq!(w.len(), 4, "the offer and three parts");
    assert!(sent(&mut t, w[0]).is_empty());
    assert!(sent(&mut t, w[1]).is_empty());
    let (conn, msg_id) = w[2];
    let err = AgentFailure::Network("quota".into());
    let out = t.agent(AgentEvent::Merr { conn, msg_id, err });
    assert_eq!(statuses(&out), [(23, None, failed("quota"))]);
    assert!(t.e.send_tags.is_empty(), "the last part says nothing more");
    assert!(sent(&mut t, w[3]).is_empty());
}

#[test]
fn in_a_group_each_member_is_told_once_and_one_whose_offer_failed_gets_no_sent() {
    let (mut t, fake) = setup();
    fake.with(|s| s.hold = true);
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Owner);
    let out = t.req(tagged(24, file(&t, ChatRef::Group(g))));
    let item = messages(&out)[0].id;
    let (bob, carol) = (ms[0], ms[1]);
    let offer = |t: &T, c: ConnId| waiting(t).into_iter().find(|(x, _)| *x == c).unwrap();
    let (conn, msg_id) = offer(&t, bob.1);
    let err = AgentFailure::Network("gone".into());
    let out = t.agent(AgentEvent::Merr { conn, msg_id, err });
    assert_eq!(statuses(&out), [(24, Some(bob.0), failed("gone"))]);
    let o = offer(&t, carol.1);
    assert!(sent(&mut t, o).is_empty());
    let job = fake.with(|s| s.jobs[0]);
    let key = FileKey::of(ChatRef::Group(g), item);
    let result = Ok(SentToMembers {
        descriptions: vec!["for bob".into(), "for carol".into()],
        expires: None,
    });
    t.e.on_job(JobDone::File(FileDone::UploadedForMembers {
        key,
        job,
        result,
    }));
    let w = waiting(&t);
    assert_eq!(w.len(), 1, "bob's description is not tagged");
    assert_eq!(sent(&mut t, w[0]), [(24, Some(carol.0), SendStatus::Sent)]);
}
