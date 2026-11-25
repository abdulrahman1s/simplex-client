//! A tagged file to the official CLI 7.1.4 is told sent once, after its upload, not with its offer; run as `live_cli` says.

use std::time::Duration;

use crate::types::model::{ChatRef, FileState, MessageBody, SendStatus};
use crate::types::{Event, FileKind, Request};

use super::Dir;
use super::live_cli::{Native, connect, until, var};
use crate::agent::tests::cli::Cli;

fn statuses(n: &Native) -> Vec<SendStatus> {
    n.events
        .iter()
        .filter_map(|e| match e {
            Event::SendStatus { tag: 5, status, .. } => Some(status.clone()),
            _ => None,
        })
        .collect()
}

/// Where in what the engine said the file showed as sent (its upload done).
fn uploaded_at(n: &Native) -> Option<usize> {
    n.events.iter().position(|e| match e {
        Event::Updated(m) => m
            .iter()
            .any(|m| matches!(&m.body, MessageBody::File(f) if f.state == FileState::Sent)),
        _ => false,
    })
}

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn a_tagged_file_is_told_sent_once_after_its_upload() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (_, bob) = connect(&mut n, &mut c);
    let path = dir.0.join("files").join("report.bin");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, vec![7u8; 200_000]).unwrap();
    n.events.clear();
    let send = Request::SendFile {
        chat: ChatRef::Direct(bob),
        path,
        kind: FileKind::File,
        preview: None,
        duration: None,
        text: String::new(),
    };
    n.req(Request::Tagged {
        tag: 5,
        request: Box::new(send),
    });
    assert!(matches!(
        statuses(&n)[..],
        [SendStatus::Queued { item: Some(_) }]
    ));
    until(&mut n, &mut c, "the file's Sent", |n, _| {
        statuses(n).contains(&SendStatus::Sent).then_some(())
    });
    let sent_at = n.events.iter().position(|e| {
        matches!(
            e,
            Event::SendStatus {
                tag: 5,
                status: SendStatus::Sent,
                ..
            }
        )
    });
    let up = uploaded_at(&n).expect("the upload finished");
    assert!(
        up < sent_at.unwrap(),
        "Sent comes after the upload, not with the offer"
    );
    for _ in 0..200 {
        n.step(Duration::from_millis(10));
        c.poll(Duration::from_millis(5));
    }
    let told = statuses(&n);
    assert_eq!(told.len(), 2, "queued, then one sent: {told:?}");
}
