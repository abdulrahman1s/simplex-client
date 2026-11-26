//! Channel traffic as official simplex-chat 7.1.4 binaries sent it, captured on local servers: an official relay (`--relay`) to a subscriber and to an owner. Each decodes to its typed event, encodes back byte for byte, and the signed ones verify under the keys the same traffic carries (Protocol.hs `x.grp.relay.*`, `x.grp.roster*`, `SignedMsg`, `GrpMsgForward`; Internal.hs `groupBindingData`).
use simplex_client::agent::links::{ConnShortLink, ContactConnType};
use simplex_client::chat::channel::{GroupType, RelayCapabilities};
use simplex_client::chat::element::{encode_signed_element, parse_element};
use simplex_client::chat::event::FileChunk;
use simplex_client::chat::forward::FwdSender;
use simplex_client::chat::group::GroupMemberRole;
use simplex_client::chat::ids::{MemberId, b64_decode};
use simplex_client::chat::signed::{ChatBinding, SignedMsg};
use simplex_client::chat::{ChatEvent as E, ChatMessage, parse_chat_elements};
use simplex_client::crypto::{Alg, PublicKey, Signature};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const LINK_INV_CONF: &str = "eyJ2IjoiOS0yMiIsImV2ZW50IjoieC5ncnAubGluay5pbnYiLCJwYXJhbXMiOnsiZ3JvdXBMaW5rSW52aXRhdGlvbiI6eyJhY2NlcHRlZCI6ImFjY2VwdGVkIiwiZnJvbU1lbWJlciI6eyJtZW1iZXJJZCI6ImVVVnNjV2QwVW1KUlUwbFhOMUJvYmc9PSIsIm1lbWJlclJvbGUiOiJyZWxheSJ9LCJmcm9tTWVtYmVyS2V5IjoiTUNvd0JRWURLMlZ3QXlFQXFMN1YzUF9XU1czSXdiLVNaTHZwY2RqLTlxY3pub1pjajZlUHVuTzlmRkE9IiwiZnJvbU1lbWJlck5hbWUiOiJyZWxheTEiLCJncm91cFByb2ZpbGUiOnsiZGlzcGxheU5hbWUiOiJvMjY5MDA2IiwiZnVsbE5hbWUiOiIiLCJncm91cFByZWZlcmVuY2VzIjp7ImRpcmVjdE1lc3NhZ2VzIjp7ImVuYWJsZSI6Im9uIn0sImhpc3RvcnkiOnsiZW5hYmxlIjoib24ifSwic3VwcG9ydCI6eyJlbmFibGUiOiJvZmYifX0sInB1YmxpY0dyb3VwIjp7Imdyb3VwTGluayI6Imh0dHBzOi8vMTI3LjAuMC4xL2MjWml1cWl0SkRmLW54T245UWxkZkpSZEdCSDduRXNKOTNOUU1za1htdW9ZST9wPTUyMjMmYz0xdUlrTkZIM3Y5bV9wMGtwUzltWTJkTmRtWC1VSjNZcWZQMFc3al9RUUlnIiwiZ3JvdXBUeXBlIjoiY2hhbm5lbCIsInB1YmxpY0dyb3VwSWQiOiJWX0xmM2k3WFk5SWQ0ZVV1YW5Ldlh2WlJtWmctRUFnN2MxOU9IVk5oU0xZPSJ9fSwiZ3JvdXBTaXplIjoyLCJpbnZpdGVkTWVtYmVyIjp7Im1lbWJlcklkIjoiYkhwS1VsZEVVa1pSTVhab1QwRTRWQT09IiwibWVtYmVyUm9sZSI6Im9ic2VydmVyIn19fX0=";
const LINK_MEM: &str = "L0cBTUCFo1AG_b_9DkeeysK5M8b5ZIsianCdbbBhZWvrrs9kLACCmksd9Gs3ImNOQkKpMDgytCXyO--3FVBJ4TChiFQLeyJ2IjoiOS0yMiIsIm1zZ0lkIjoiY0RSMGNqTkxha2ROUVZaclpqUk5iUT09IiwiZXZlbnQiOiJ4LmdycC5saW5rLm1lbSIsInBhcmFtcyI6eyJtZW1iZXJLZXkiOiJNQ293QlFZREsyVndBeUVBcUw3VjNQX1dTVzNJd2ItU1pMdnBjZGotOXFjem5vWmNqNmVQdW5POWZGQT0iLCJwcm9maWxlIjp7ImRpc3BsYXlOYW1lIjoicmVsYXkxIiwiZnVsbE5hbWUiOiIifX19";
const MEM_INTRO: &str = "eyJ2IjoiOS0yMiIsIm1zZ0lkIjoiTUZKNGMwSlphMUJYWjNCamVIWlRSQT09IiwiZXZlbnQiOiJ4LmdycC5tZW0uaW50cm8iLCJwYXJhbXMiOnsibWVtYmVySW5mbyI6eyJtZW1iZXJJZCI6Ik0wSldhalE0VTFkSFFWWnNaMGhhUkE9PSIsIm1lbWJlcktleSI6Ik1Db3dCUVlESzJWd0F5RUFHRjNmTF9uWExMNG92Z3VwS0ZhMUJRWG04WWt1Sjh5b2MwVGpta0h1OGNVPSIsIm1lbWJlclJvbGUiOiJvd25lciIsInByb2ZpbGUiOnsiZGlzcGxheU5hbWUiOiJvd25lciIsImZ1bGxOYW1lIjoiIn0sInYiOiI5LTIyIn19fQ==";
const ROSTER_FWD: &str = "PQEBRD5NEDNCVmo0OFNXR0FWbGdIWkQFb3duZXIAAAAAawgk7S9HAU1Ax-6_y4LRbY4xRuSCLOrEiXl2hXDn0AWue1-dPACVEZ3azeLNBfcXaP9LMR3jPoOtMdMEkrPf2xjIRw5lYOapBHsidiI6IjktMjIiLCJtc2dJZCI6IlIyOVZlVlJEVUhCR1UxSm9Wa1J1TWc9PSIsImV2ZW50IjoieC5ncnAucm9zdGVyIiwicGFyYW1zIjp7ImZpbGVJbnYiOnsiZmlsZURpZ2VzdCI6IlhxY2R4dEMwOVh2em1xM1FmQ0NNTmZCczByckZfZUlRT1g5dzNoSFVPY1l1d2MzekdEZFlobF9UaF96cUM2Mmk5c042U2hlRkhkSFhqLV9tOGdUdVZBPT0iLCJmaWxlU2l6ZSI6Mn0sInZlcnNpb24iOjB9fQ==";
const ROSTER_CHUNK: &str = "RgAQR29VeVRDUHBGU1JoVkRuMgEAAA==";
const HISTORY_BATCH: &str = "PQMAsD5DAAAAAGsIJO17ImV2ZW50IjoieC5tc2cubmV3IiwibXNnSWQiOiJiR280Vm5GTllVZE1abWhMTURSaFdnPT0iLCJwYXJhbXMiOnsiYXNHcm91cCI6dHJ1ZSwiY29udGVudCI6eyJhbHBoYSI6eyJhIjoxLCJiIjoyfSwidGV4dCI6InBvc3QiLCJ0eXBlIjoienouYXBwIiwiemV0YSI6MX19LCJ2IjoiOS0yMiJ9ALA-QwAAAABrCCTteyJldmVudCI6IngubXNnLm5ldyIsIm1zZ0lkIjoiVkZsaksxbG5URklyVkZKelprWlJOUT09IiwicGFyYW1zIjp7ImFzR3JvdXAiOnRydWUsImNvbnRlbnQiOnsiYWxwaGEiOnsiYSI6MSwiYiI6Mn0sInRleHQiOiJwb3N0IiwidHlwZSI6Inp6LmFwcCIsInpldGEiOjJ9fSwidiI6IjktMjIifQCwPkMAAAAAawgk7XsiZXZlbnQiOiJ4Lm1zZy5uZXciLCJtc2dJZCI6ImNIZGtkMHh6VW5SUmFVSXhZWHBIT1E9PSIsInBhcmFtcyI6eyJhc0dyb3VwIjp0cnVlLCJjb250ZW50Ijp7ImFscGhhIjp7ImEiOjEsImIiOjJ9LCJ0ZXh0IjoicG9zdCIsInR5cGUiOiJ6ei5hcHAiLCJ6ZXRhIjozfX0sInYiOiI5LTIyIn0=";
const POST_DATA: &str = "PQEAsD5DAAAAAGsIJO17ImV2ZW50IjoieC5tc2cubmV3IiwibXNnSWQiOiJNbVowYUdOS2NqUXhkM05QV0V4eFR3PT0iLCJwYXJhbXMiOnsiYXNHcm91cCI6dHJ1ZSwiY29udGVudCI6eyJhbHBoYSI6eyJhIjoxLCJiIjoyfSwidGV4dCI6InBvc3QiLCJ0eXBlIjoienouYXBwIiwiemV0YSI6NH19LCJ2IjoiOS0yMiJ9";
const POST_TEXT: &str = "PQEAnD5DAAAAAGsIJO17ImV2ZW50IjoieC5tc2cubmV3IiwibXNnSWQiOiJaMDlwVEVSSlZISlFSVkJWZEd0WU53PT0iLCJwYXJhbXMiOnsiYXNHcm91cCI6dHJ1ZSwiY29udGVudCI6eyJ0ZXh0IjoiaGVsbG8gc3Vic2NyaWJlcnMiLCJ0eXBlIjoidGV4dCJ9fSwidiI6IjktMjIifQ==";
const RELAY_ACPT_CONF: &str = "eyJ2IjoiOS0yMiIsImV2ZW50IjoieC5ncnAucmVsYXkuYWNwdCIsInBhcmFtcyI6eyJyZWxheUNhcCI6e30sInJlbGF5TGluayI6Imh0dHBzOi8vMTI3LjAuMC4xL2cjQmN4RTQyMFpyejEwR25jS1ZwUllKOWVHZXRlWERZWC12SkpQeEo1Nm5YTT9wPTUyMjMmYz0xdUlrTkZIM3Y5bV9wMGtwUzltWTJkTmRtWC1VSjNZcWZQMFc3al9RUUlnIn19";
const ROSTER_ACK: &str = "eyJ2IjoiOS0yMiIsIm1zZ0lkIjoiWmpsSE4ySkxhRVoyYUhsSFF6SjBNdz09IiwiZXZlbnQiOiJ4LmdycC5yb3N0ZXIuYWNrIiwicGFyYW1zIjp7InZlcnNpb24iOjB9fQ==";
const MEM_NEW: &str = "eyJ2IjoiOS0yMiIsIm1zZ0lkIjoiV25GQk4zY3lTWEU0UTBOck5tUlNjdz09IiwiZXZlbnQiOiJ4LmdycC5tZW0ubmV3IiwicGFyYW1zIjp7Im1lbWJlckluZm8iOnsibWVtYmVySWQiOiJURmhITTB0U2VVNUZTVGczTms1S1JBPT0iLCJtZW1iZXJLZXkiOiJNQ293QlFZREsyVndBeUVBNy1nNHJkblNnbUZsdDFUd2lJOEgtWHN0dVlyeTNZcll5YlBSRzhsM0g4MD0iLCJtZW1iZXJSb2xlIjoib2JzZXJ2ZXIiLCJwcm9maWxlIjp7ImRpc3BsYXlOYW1lIjoic3ViMSIsImZ1bGxOYW1lIjoiIn0sInYiOiI5LTIyIn19fQ==";

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// What the official owner CLI itself sent its relay (its own message table): the roster
/// header, its blob chunk, and a text post as the channel.
const OWNER_ROSTER: &str = "eyJ2IjoiOS0yMiIsIm1zZ0lkIjoiSzJadk5ETlBPVTh3V2pkSGFDOXBNdz09IiwiZXZlbnQiOiJ4LmdycC5yb3N0ZXIiLCJwYXJhbXMiOnsiZmlsZUludiI6eyJmaWxlRGlnZXN0IjoiWHFjZHh0QzA5WHZ6bXEzUWZDQ01OZkJzMHJyRl9lSVFPWDl3M2hIVU9jWXV3YzN6R0RkWWhsX1RoX3pxQzYyaTlzTjZTaGVGSGRIWGotX204Z1R1VkE9PSIsImZpbGVTaXplIjoyfSwidmVyc2lvbiI6MH19";
const OWNER_ROSTER_CHUNK: &str = "RgAQK2ZvNDNPOU8wWjdHaC9pMwEAAA==";
const OWNER_POST: &str = "eyJ2IjoiOS0yMiIsIm1zZ0lkIjoiZW14U01YTnRWVGtyVGxVd1RETkhUdz09IiwiZXZlbnQiOiJ4Lm1zZy5uZXciLCJwYXJhbXMiOnsiYXNHcm91cCI6dHJ1ZSwiY29udGVudCI6eyJ0ZXh0IjoiaGVsbG8gY2hhbm5lbCIsInR5cGUiOiJ0ZXh0In19fQ==";

fn bytes(s: &str) -> Vec<u8> {
    b64_decode(s).unwrap()
}

fn only(s: &str) -> simplex_client::chat::ParsedMsg {
    let mut all = parse_chat_elements(&bytes(s));
    assert_eq!(all.len(), 1);
    all.remove(0).unwrap()
}

/// `smpEncode (publicGroupId, memberId)`.
fn binding(public_group_id: &[u8], member_id: &MemberId) -> Vec<u8> {
    let mut out = Vec::new();
    for part in [public_group_id, member_id.as_bytes()] {
        out.push(u8::try_from(part.len()).unwrap());
        out.extend_from_slice(part);
    }
    out
}

fn verifies(key_b64: &str, binding: &[u8], sm: &SignedMsg) -> bool {
    let key = PublicKey::from_der(&b64_decode(key_b64).unwrap()).unwrap();
    let input = sm.signed_input(binding);
    sm.signatures.iter().all(|s| {
        key.verify(
            &Signature::from_raw(Alg::Ed25519, &s.signature).unwrap(),
            &input,
        )
    })
}

/// The relay's `x.grp.link.inv`: who the relay is in the channel, and the channel's identity.
fn link_inv() -> simplex_client::chat::group::GroupLinkInvitation {
    match only(LINK_INV_CONF).message.event {
        E::GrpLinkInv(i) => i,
        other => panic!("{other:?}"),
    }
}

fn public_group_id() -> Vec<u8> {
    let pg = link_inv().group_profile.public_group_profile().unwrap();
    b64_decode(&pg.public_group_id).unwrap()
}

#[test]
fn a_relay_s_link_invitation_names_a_channel_and_an_observer() {
    let inv = link_inv();
    assert_eq!(inv.from_member.member_role, GroupMemberRole::Relay);
    assert_eq!(inv.invited_member.member_role, GroupMemberRole::Observer);
    let pg = inv.group_profile.public_group_profile().unwrap();
    assert_eq!(pg.group_type, GroupType::Channel);
    let link = ConnShortLink::parse(&pg.group_link).unwrap();
    assert!(matches!(
        link,
        ConnShortLink::Contact {
            contact_type: ContactConnType::Channel,
            ..
        }
    ));
    assert_eq!(public_group_id().len(), 32);
    let m = only(LINK_INV_CONF).message;
    assert_eq!(m.to_json().unwrap(), bytes(LINK_INV_CONF));
}

#[test]
fn a_relay_s_signed_link_member_profile_verifies_under_its_channel_binding() {
    let p = only(LINK_MEM);
    let sm = p.signed.clone().unwrap();
    assert_eq!(sm.binding, ChatBinding::Group);
    assert_eq!(encode_signed_element(&sm).unwrap(), bytes(LINK_MEM));
    let E::GrpLinkMem {
        member_key: Some(key),
        ..
    } = &p.message.event
    else {
        panic!("{:?}", p.message.event)
    };
    let inv = link_inv();
    assert_eq!(inv.from_member_key.as_deref(), Some(key.as_str()));
    let b = binding(&public_group_id(), &inv.from_member.member_id);
    assert!(verifies(key, &b, &sm));
    // Bound to the channel: the classic `(memberId, memberKey)` binding does not verify.
    let classic = binding(
        inv.from_member.member_id.as_bytes(),
        &MemberId(b64_decode(key).unwrap()),
    );
    assert!(!verifies(key, &classic, &sm));
}

#[test]
fn an_owner_s_roster_forwarded_by_the_relay_keeps_its_signed_bytes() {
    let E::GrpMemIntro {
        member_info: owner, ..
    } = only(MEM_INTRO).message.event
    else {
        panic!()
    };
    assert_eq!(owner.member_role, GroupMemberRole::Owner);
    assert_eq!(only(MEM_INTRO).message.to_json().unwrap(), bytes(MEM_INTRO));
    let mut batch = parse_chat_elements(&bytes(ROSTER_FWD));
    let p = batch.remove(0).unwrap();
    let fwd = p.forward.unwrap();
    assert!(
        matches!(&fwd.sender, FwdSender::Member { member_id, .. } if *member_id == owner.member_id)
    );
    let sm = p.signed.unwrap();
    let E::GrpRoster {
        version: 0,
        file_inv,
    } = &p.message.event
    else {
        panic!("{:?}", p.message.event)
    };
    assert_eq!(file_inv.file_size, 2);
    let b = binding(&public_group_id(), &owner.member_id);
    assert!(verifies(owner.member_key.as_deref().unwrap(), &b, &sm));
    let E::FileChunk {
        msg_id,
        chunk: FileChunk::Chunk {
            chunk_no: 1,
            bytes: blob,
        },
    } = only(ROSTER_CHUNK).message.event
    else {
        panic!()
    };
    assert_eq!(Some(msg_id), p.message.msg_id);
    assert_eq!(blob, [0, 0], "an empty roster is the Word16 count 0");
}

#[test]
fn history_and_posts_as_the_channel_come_forwarded_from_nobody_named() {
    let history = parse_chat_elements(&bytes(HISTORY_BATCH));
    assert_eq!(history.len(), 3);
    for p in history {
        let p = p.unwrap();
        assert_eq!(
            p.forward.as_ref().map(|f| &f.sender),
            Some(&FwdSender::Channel)
        );
        assert!(p.signed.is_none());
        let E::MsgNew(c) = &p.message.event else {
            panic!()
        };
        assert_eq!(c.as_group, Some(true));
        // The relay re-encoded the unsigned post, its keys in Aeson's sorted order.
        let shared = simplex_client::chat::ids::b64_encode(&p.message.msg_id.unwrap().0);
        let relayed =
            format!(r#"{{"event":"x.msg.new","msgId":"{shared}","params":{{"asGroup":true,"#);
        assert!(contains(&bytes(HISTORY_BATCH), relayed.as_bytes()));
    }
    for raw in [POST_DATA, POST_TEXT] {
        let el = parse_chat_elements(&bytes(raw)).remove(0).unwrap();
        assert_eq!(el.forward.unwrap().sender, FwdSender::Channel);
    }
}

#[test]
fn an_unknown_content_type_reaches_a_subscriber_whole_with_sorted_keys() {
    let el = parse_chat_elements(&bytes(POST_DATA)).remove(0).unwrap();
    let json = String::from_utf8(el.message.to_json().unwrap()).unwrap();
    assert!(
        json.contains(
            r#""content":{"alpha":{"a":1,"b":2},"text":"post","type":"zz.app","zeta":4}"#
        ),
        "{json}"
    );
}

#[test]
fn a_relay_accepts_with_its_link_for_the_channel_and_answers_the_roster() {
    let m = only(RELAY_ACPT_CONF).message;
    let E::GrpRelayAcpt {
        relay_link,
        relay_cap,
    } = &m.event
    else {
        panic!("{:?}", m.event)
    };
    assert_eq!(*relay_cap, RelayCapabilities::default());
    assert!(matches!(
        ConnShortLink::parse(relay_link).unwrap(),
        ConnShortLink::Contact {
            contact_type: ContactConnType::Group,
            ..
        }
    ));
    assert_eq!(m.to_json().unwrap(), bytes(RELAY_ACPT_CONF));
    let ack = only(ROSTER_ACK).message;
    assert_eq!(
        ack.event,
        E::GrpRosterAck {
            version: 0,
            error: None
        }
    );
    assert_eq!(ack.to_json().unwrap(), bytes(ROSTER_ACK));
    let new = only(MEM_NEW).message;
    let E::GrpMemNew { member_info, .. } = &new.event else {
        panic!()
    };
    assert_eq!(member_info.member_role, GroupMemberRole::Observer);
    assert_eq!(new.to_json().unwrap(), bytes(MEM_NEW));
}

#[test]
fn every_captured_element_parses() {
    for s in [
        LINK_INV_CONF,
        LINK_MEM,
        MEM_INTRO,
        ROSTER_FWD,
        ROSTER_CHUNK,
        HISTORY_BATCH,
        POST_DATA,
        POST_TEXT,
        RELAY_ACPT_CONF,
        ROSTER_ACK,
        MEM_NEW,
    ] {
        for p in parse_chat_elements(&bytes(s)) {
            p.unwrap();
        }
    }
    let _: Option<ChatMessage> = None;
    let _ = parse_element;
}

#[test]
fn the_crate_encodes_an_owner_s_empty_roster_and_its_blob_as_the_official_owner_does() {
    use simplex_client::chat::channel::InlineFileInvitation;
    use simplex_client::chat::element::encode_file_chunk_element;
    use simplex_client::chat::ids::{SharedMsgId, b64_encode};
    use simplex_client::chat::version::SUPPORTED_CHAT_VRANGE;
    let theirs = only(OWNER_ROSTER).message;
    let blob = [0u8, 0];
    let ours = ChatMessage {
        v: Some(SUPPORTED_CHAT_VRANGE),
        msg_id: theirs.msg_id.clone(),
        event: E::GrpRoster {
            version: 0,
            file_inv: InlineFileInvitation {
                file_size: 2,
                file_digest: b64_encode(&simplex_client::crypto::sha512(&blob)),
            },
        },
    };
    assert_eq!(ours.to_json().unwrap(), bytes(OWNER_ROSTER));
    let id: SharedMsgId = theirs.msg_id.unwrap();
    let chunk = FileChunk::Chunk {
        chunk_no: 1,
        bytes: blob.to_vec(),
    };
    assert_eq!(
        encode_file_chunk_element(&id, &chunk).unwrap(),
        bytes(OWNER_ROSTER_CHUNK)
    );
}

#[test]
fn the_crate_encodes_a_post_as_the_channel_as_the_official_owner_does() {
    use simplex_client::chat::container::MsgContainer;
    use simplex_client::chat::version::SUPPORTED_CHAT_VRANGE;
    let theirs = only(OWNER_POST).message;
    let mut c = MsgContainer::text("hello channel");
    c.as_group = Some(true);
    let ours = ChatMessage {
        v: Some(SUPPORTED_CHAT_VRANGE),
        msg_id: theirs.msg_id.clone(),
        event: E::MsgNew(c),
    };
    assert_eq!(ours.to_json().unwrap(), bytes(OWNER_POST));
}
