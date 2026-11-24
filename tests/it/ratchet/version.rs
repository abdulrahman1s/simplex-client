//! A ratchet's current version follows the highest the peer offers, up to its own (Ratchet.hs `rcDecrypt` `upgradedRatchet`, `rcEncryptHeader`).
use simplex_client::encoding::Version;
use simplex_client::ratchet::{CURRENT_E2E_VERSION, RatchetVersions};

use super::support::{PADDED, handshake, handshake_with, recv, reload, send};

#[test]
fn a_peer_offering_a_later_version_raises_current_up_to_ours_and_it_is_offered_back() {
    let v = RatchetVersions {
        current: CURRENT_E2E_VERSION,
        max_supported: Version(4),
    };
    let mut p = handshake_with(false, false, Some(v));
    let key = p.bob.encrypt_header(None, Version(4)).unwrap();
    let wire = key.encrypt_msg(PADDED, b"hi").unwrap();
    assert_eq!(p.alice.decrypt(&wire).unwrap(), b"hi");
    assert_eq!(p.alice.versions().current, Version(4));
    assert_eq!(reload(&p.alice).versions().current, Version(4), "stored");
    // Alice now sends at v4 and offers it, so Bob follows.
    let wire = send(&mut p.alice, "back");
    assert_eq!(recv(&mut p.bob, &wire), "back");
    assert_eq!(p.bob.versions().current, Version(4));
}

#[test]
fn a_later_version_than_we_speak_leaves_current_where_it_is() {
    let mut p = handshake(false, false);
    let key = p.bob.encrypt_header(None, Version(9)).unwrap();
    let wire = key.encrypt_msg(PADDED, b"hi").unwrap();
    assert_eq!(p.alice.decrypt(&wire).unwrap(), b"hi");
    assert_eq!(p.alice.versions().current, CURRENT_E2E_VERSION);
}
