//! `Client` refuses what breaks a stateless limit before it reaches the engine.

use super::super::Error;
use super::{builder, start};
use crate::engine::tests::{Dir, FakeAgent};
use crate::types::model::ChatRef;
use crate::types::{ErrorCode, Request, RequestKind};

fn refused(r: Result<(), Error>) -> Option<(RequestKind, ErrorCode)> {
    match r {
        Err(Error::Failed { request, code, .. }) => Some((request, code)),
        Ok(()) | Err(_) => None,
    }
}

#[test]
fn a_request_breaking_a_limit_fails_at_once_with_its_code() {
    let dir = Dir::new();
    let (client, _net) = start(builder(&dir), FakeAgent::default());
    let react = Request::React {
        chat: ChatRef::Direct(1),
        item: 1,
        emoji: "🤡".into(),
        on: true,
    };
    assert_eq!(
        refused(client.request(react)),
        Some((RequestKind::React, ErrorCode::ReactionNotAllowed))
    );
    let group = client.request(Request::NewGroup("#club".into()));
    assert_eq!(
        refused(group),
        Some((RequestKind::Group, ErrorCode::InvalidName))
    );
    let long = "x".repeat(crate::limits::MAX_ENCODED_MSG_LENGTH + 1);
    assert_eq!(
        refused(client.send(ChatRef::Direct(1), &long)),
        Some((RequestKind::Send, ErrorCode::TextTooLong))
    );
    client.close();
}

#[test]
fn a_new_store_refuses_a_name_upstream_would_change() {
    let dir = Dir::new();
    let started = builder(&dir)
        .name("J.Doe")
        .start_with(Box::new(crate::engine::FakeClock::new(0)), |_, _| {
            (FakeAgent::default(), crossbeam_channel::unbounded().1)
        });
    assert!(matches!(
        started,
        Err(Error::Failed {
            request: RequestKind::Start,
            code: ErrorCode::InvalidName,
            ..
        })
    ));
}

#[test]
fn an_existing_store_starts_whatever_name_the_builder_has() {
    let dir = Dir::new();
    let (client, _net) = start(builder(&dir), FakeAgent::default());
    client.close();
    let (again, _net) = start(builder(&dir).name("J.Doe"), FakeAgent::default());
    assert_eq!(again.profile().name, "alice");
    again.close();
}
