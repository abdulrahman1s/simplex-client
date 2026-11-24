//! The pure core and its JS bindings on wasm32 under node: randomness, time, the ratchet, chat messages and links.
#![cfg(all(target_arch = "wasm32", feature = "wasm"))]
#![allow(clippy::unwrap_used, reason = "tests")]

use serde_json::{Value, json};
use simplex_client::crypto::{Alg, KeyPair};
use simplex_client::ratchet::{
    CURRENT_E2E_VERSION, ParamsUri, PqSupport, Ratchet, RatchetVersions, compatible_e2e_version,
    generate_rcv_params, generate_snd_params, pq_x3dh_rcv, pq_x3dh_snd, reply_kem,
    supported_e2e_range,
};
use simplex_client::smp::{Clock, SystemClock};
use simplex_client::wasm;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test as test;

const INVITATION_SHORT: &str = "https://smp5.simplex.im/i#esW_-BXJI9zs4iGRV2SMHUzqpFv3QRe6/KaKvW0lOkJzcvIRVW-l0uomDrjYKjiaOkPIkxSvOB0o";
const CONTACT_FULL: &str = "simplex:/contact#/?v=6-8&smp=smp%3A%2F%2FPQUV2eL0t7OStZOoAsPEV2QYWt4-xilbakvGUGOItUo%3D%40smp6.simplex.im%2FUKN0gf7WnXXNs1k0FUleanVhaQysygYI%23%2F%3Fv%3D1-4%26dh%3DMCowBQYDK2VuAyEApIv2f5DPQiNCNcOUciZAOS3jl45pQxZiG4RwENuJlDE%253D%26q%3Dc%26srv%3Dbylepyau3ty4czmn77q4fglvperknl4bi2eb2fdy2bh4jxtf32kf73yd.onion";
const GROUP_SHORT: &str = "https://smp6.simplex.im/g#0sPWloEa5zFH0sfc_vANe4RUUebBQSGcvTKpKRH300s";

fn from_js(v: &JsValue) -> Value {
    serde_json::from_str(&js_sys::JSON::stringify(v).unwrap().as_string().unwrap()).unwrap()
}

#[test]
fn randomness_comes_from_the_js_crypto_source() {
    let (a, b) = (
        wasm::random_bytes(32).unwrap(),
        wasm::random_bytes(32).unwrap(),
    );
    assert_eq!(a.len(), 32);
    assert_ne!(a, b);
    assert_ne!(a, [0u8; 32]);
}

#[test]
fn the_smp_clock_reads_the_js_clock_without_panicking() {
    let clock = SystemClock::new();
    let first = clock.now_ms();
    assert!(first < 1000, "a fresh clock starts near zero, got {first}");
    assert!(clock.now_ms() >= first);
}

#[test]
fn generated_keys_sign_and_verify() {
    for alg in ["ed25519", "ed448"] {
        let keys = wasm::generate_key_pair(alg).unwrap();
        let sig = wasm::sign(&keys.private_key, b"hello").unwrap();
        assert!(wasm::verify(&keys.public_key, &sig, b"hello"));
        assert!(!wasm::verify(&keys.public_key, &sig, b"other"));
    }
    assert!(wasm::generate_key_pair("x448").is_ok());
    assert!(
        wasm::sign(
            &wasm::generate_key_pair("x25519").unwrap().private_key,
            b"x"
        )
        .is_err()
    );
}

#[test]
fn a_post_quantum_ratchet_round_trips_both_ways() {
    let (a_priv, a_params) = generate_rcv_params(CURRENT_E2E_VERSION, PqSupport(true)).unwrap();
    let uri =
        ParamsUri::from_query(&ParamsUri::from_params(&a_params, supported_e2e_range()).to_query())
            .unwrap();
    let v = compatible_e2e_version(&uri.vr).unwrap();
    let rcv = uri.to_rcv_params(v).unwrap();
    let use_kem = reply_kem(rcv.kem_key(), PqSupport(true));
    let (b_priv, b_params) = generate_snd_params(v, use_kem.as_ref()).unwrap();
    let bob_init = pq_x3dh_snd(&b_priv, &rcv).unwrap();
    let alice_init = pq_x3dh_rcv(&a_priv, &b_params).unwrap();
    let versions = RatchetVersions::new(v);
    let dhrs = KeyPair::generate(Alg::X448).unwrap().private;
    let mut bob = Ratchet::init_snd(versions, rcv.params().k2.clone(), dhrs, bob_init).unwrap();
    let mut alice = Ratchet::init_rcv(versions, a_priv.pk2, alice_init, PqSupport(true));

    let wire = bob.encrypt(b"hello alice", 100, None).unwrap();
    assert_eq!(alice.decrypt(&wire).unwrap(), b"hello alice");
    let wire = alice.encrypt(b"hello bob", 100, None).unwrap();
    assert_eq!(bob.decrypt(&wire).unwrap(), b"hello bob");
}

#[test]
fn chat_messages_encode_and_parse_through_the_bindings() {
    let msg = json!({
        "v": "1-16",
        "msgId": "AAAAAAAAAAAAAAAA",
        "event": "x.msg.new",
        "params": {"content": {"type": "text", "text": "hello!"}}
    });
    let body = wasm::encode_message(&msg.to_string(), false).unwrap();
    assert_eq!(body[0], b'{');
    let back = from_js(&wasm::parse_messages(&body).unwrap());
    assert_eq!(back[0]["params"]["content"]["text"], "hello!");
    assert_eq!(back[0]["msgId"], "AAAAAAAAAAAAAAAA");
}

#[test]
fn a_long_chat_message_is_compressed_and_parses_back() {
    let text = "hello json ".repeat(3000);
    let msg = json!({"event": "x.msg.new", "params": {"content": {"type": "text", "text": text}}});
    let body = wasm::encode_message(&msg.to_string(), true).unwrap();
    assert_eq!(body[0], b'X');
    let back = from_js(&wasm::parse_messages(&body).unwrap());
    assert_eq!(back[0]["params"]["content"]["text"], text.as_str());
}

#[test]
fn links_report_their_kind_and_servers() {
    let inv = from_js(&wasm::parse_link(INVITATION_SHORT).unwrap());
    assert_eq!(
        (&inv["kind"], &inv["short"]),
        (&json!("invitation"), &json!(true))
    );
    assert!(
        inv["servers"][0]
            .as_str()
            .unwrap()
            .contains("smp5.simplex.im")
    );
    assert_eq!(inv["link"], INVITATION_SHORT);

    let contact = from_js(&wasm::parse_link(CONTACT_FULL).unwrap());
    assert_eq!(
        (&contact["kind"], &contact["short"]),
        (&json!("contact"), &json!(false))
    );
    let server = contact["servers"][0].as_str().unwrap();
    assert!(
        server.starts_with("smp://PQUV2eL0t7OStZOoAsPEV2QYWt4-xilbakvGUGOItUo=@smp6.simplex.im")
    );

    assert_eq!(
        from_js(&wasm::parse_link(GROUP_SHORT).unwrap())["kind"],
        "group"
    );
    assert!(wasm::parse_link("https://example.com/nothing").is_err());
}
