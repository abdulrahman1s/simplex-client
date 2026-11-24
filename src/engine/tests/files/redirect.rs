//! Files: redirect descriptions are refused, and a download may fetch no more than offered.

use super::*;
use crate::xftp::description::Redirect;

fn redirect_to(server: &XftpServer) -> String {
    let mut fd = FileDescription::parse_valid(&description(server)).unwrap();
    fd.redirect = Some(Redirect {
        size: 65536,
        digest: vec![9; 64],
    });
    encode_description(&fd)
}

#[test]
fn a_redirect_description_is_refused_and_nothing_is_fetched() {
    let (mut t, fake) = setup();
    let (bob, conn) = t.friend("bob");
    let m = peer_offers(&mut t, &conn, picture("p.jpg", 10 * KB));
    let shared = shared_of(&mut t, bob, m.id);
    let mut shown = Vec::new();
    for (n, p) in parts(&shared, &redirect_to(&preset_servers()[0]), 250)
        .into_iter()
        .enumerate()
    {
        shown.extend(t.peer_says(&conn, 10 + n as i64, p));
    }
    assert!(fake.with(|s| s.downloads.is_empty()));
    let last = messages(&shown).pop().unwrap();
    assert_eq!(media(&last).state, FileState::Failed);
    let out = t.req(Request::ReceiveFile {
        chat: m.chat,
        item: m.id,
    });
    assert!(failed(&out, RequestKind::File), "{out:?}");
    assert!(fake.with(|s| s.downloads.is_empty()));
}

#[test]
fn a_download_may_fetch_no_more_than_the_offer_allows() {
    let (mut t, fake) = setup();
    fake.with(|s| s.save_as = "p.jpg".into());
    let (bob, conn) = t.friend("bob");
    let m = peer_offers(&mut t, &conn, picture("p.jpg", 10 * KB));
    let shared = shared_of(&mut t, bob, m.id);
    for (n, p) in parts(&shared, &known(), 250).into_iter().enumerate() {
        t.peer_says(&conn, 10 + n as i64, p);
    }
    assert_eq!(
        fake.with(|s| s.max_sizes.clone()),
        [10 * KB + 10 * 1024 * KB]
    );
}
