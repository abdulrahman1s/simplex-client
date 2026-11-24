//! The sans-I/O HTTP/2 client driven by hand: bytes in any pieces, bytes out in bounded batches.
use simplex_client::h2::frames::*;
use simplex_client::h2::{H2Client, Options, Request, encode_headers};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

fn frames(mut bytes: &[u8]) -> Vec<(FrameHeader, Vec<u8>)> {
    let mut out = Vec::new();
    while let Some(head) = bytes.first_chunk::<HEADER_LEN>() {
        let h = parse_header(head);
        out.push((h, bytes[HEADER_LEN..HEADER_LEN + h.length].to_vec()));
        bytes = &bytes[HEADER_LEN + h.length..];
    }
    out
}

fn post(body: &[u8]) -> Request<'_> {
    Request {
        method: "POST",
        path: "/",
        authority: "a",
        headers: &[],
        body,
    }
}

#[test]
fn a_response_fed_one_byte_at_a_time_completes_the_request() {
    let mut c = H2Client::open(Options::default());
    let hello = c.take_outgoing().unwrap();
    assert!(hello.starts_with(CLIENT_PREFACE));
    c.start(&post(b"ping"), 100).unwrap();
    assert!(c.is_busy());
    let sent = frames(&c.take_outgoing().unwrap());
    assert_eq!(
        sent.iter().map(|f| f.0.kind).collect::<Vec<_>>(),
        [HEADERS, DATA]
    );
    let mut answer = encode_frame(
        HEADERS,
        END_HEADERS,
        1,
        &encode_headers(&[(":status", "200")]),
    );
    answer.extend(encode_frame(DATA, END_STREAM, 1, b"pong"));
    for b in &answer {
        assert!(c.take_response().is_none());
        c.feed(std::slice::from_ref(b)).unwrap();
    }
    let r = c.take_response().unwrap();
    assert_eq!((r.status, r.body.as_slice()), (200, &b"pong"[..]));
    assert!(!c.is_busy());
    assert_eq!(c.progress().received, 4);
}

#[test]
fn send_ahead_caps_the_body_bytes_the_peer_has_not_acknowledged() {
    let opts = Options {
        send_ahead: 20_000,
        ..Options::default()
    };
    let mut c = H2Client::open(opts);
    c.take_outgoing();
    let body = vec![7u8; 50_000];
    c.start(&post(&body), 0).unwrap();
    let data = |c: &mut H2Client| -> usize {
        let out = c.take_outgoing().unwrap_or_default();
        frames(&out)
            .iter()
            .filter(|f| f.0.kind == DATA)
            .map(|f| f.1.len())
            .sum()
    };
    assert_eq!(data(&mut c), 20_000, "the window is 65535, the cap 20000");
    assert_eq!(data(&mut c), 0);
    c.feed(&encode_window_update(1, 15_000)).unwrap();
    c.feed(&encode_window_update(0, 15_000)).unwrap();
    assert_eq!(data(&mut c), 15_000, "acknowledged bytes make room again");
}
