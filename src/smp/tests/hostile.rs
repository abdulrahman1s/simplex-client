//! Random and mutated input, fixed seed, into every parser that sees a hostile server's bytes: none may panic.
use super::certs::identity;
use crate::smp::signed_key::verify_signed_key;
use crate::smp::{Response, ServerHello, parse_payload, parse_rcv_body};
use crate::tls::{Pin, TlsClient, certificate_public_key, verify_server_chain};

/// xorshift64: reproducible, so a failure names its round.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn bytes(&mut self, max: usize) -> Vec<u8> {
        let n = self.below(max + 1);
        (0..n).map(|_| self.next() as u8).collect()
    }

    /// Small length bytes, spaces and letters, so parsers get past their first field.
    fn plausible(&mut self) -> Vec<u8> {
        let n = self.below(64);
        (0..n)
            .map(|_| match self.below(4) {
                0 => self.below(8) as u8,
                1 => b' ',
                2 => b'A' + self.below(26) as u8,
                _ => self.next() as u8,
            })
            .collect()
    }

    /// `der` with a few bytes changed, or cut short.
    fn mutate(&mut self, der: &[u8]) -> Vec<u8> {
        let mut out = der.to_vec();
        for _ in 0..1 + self.below(3) {
            let at = self.below(out.len());
            out[at] = self.next() as u8;
        }
        if self.below(4) == 0 {
            out.truncate(self.below(out.len()));
        }
        out
    }
}

const TAGS: &[&[u8]] = &[
    b"IDS ",
    b"LNK ",
    b"MSG ",
    b"SOK ",
    b"NID ",
    b"INFO ",
    b"PKEY ",
    b"PRES ",
    b"ERR ",
    b"OK",
    b"END",
    b"ERR PROXY BROKER TRANSPORT ",
    b"ERR CMD ",
    b"ERR BLOCKED ",
];

#[test]
fn smp_parsers_survive_hostile_input() {
    let mut r = Rng(0x9e37_79b9_7f4a_7c15);
    for i in 0..50_000u32 {
        let body = if i % 2 == 0 {
            r.plausible()
        } else {
            r.bytes(300)
        };
        let input = [TAGS[r.below(TAGS.len())], &body[..]].concat();
        let v = [15u16, 16, 17, 21, 22, 23][r.below(6)];
        let raw = r.bytes(600);
        let _ = Response::decode(v, &input);
        let _ = parse_payload(v, &raw);
        let _ = parse_payload(v, &[&[1u8, 0, body.len() as u8][..], &body].concat());
        let _ = ServerHello::decode(&raw);
        let _ = ServerHello::decode(&[&[0u8, 15, 0, 23][..], &body].concat());
        let _ = parse_rcv_body(&raw);
        let _ = verify_signed_key(&raw, &body);
        let _ = certificate_public_key(&raw);
    }
}

#[test]
fn certificate_checks_survive_mutated_chains() {
    let mut r = Rng(0x2545_f491_4f6c_dd1d);
    let id = identity();
    for _ in 0..20_000u32 {
        let mut chain = id.chain.clone();
        let at = r.below(chain.len());
        chain[at] = r.mutate(&chain[at]);
        let now = [0, 1_795_478_400, i64::MAX, i64::MIN][r.below(4)];
        let _ = verify_server_chain(&chain, &id.key_hash, now);
        let _ = certificate_public_key(&chain[at]);
    }
}

#[test]
fn tls_client_survives_random_records() {
    let mut r = Rng(0xd1b5_4a32_d192_ed03);
    for _ in 0..10_000u32 {
        let pin = Pin {
            key_hash: vec![1; 32],
            now: 1_795_478_400,
        };
        let (mut c, _) = TlsClient::start_pinned(&["smp/1"], pin).unwrap();
        let mut rec = vec![[22u8, 21, 20, 23][r.below(4)], 3, 3];
        let body = r.bytes(400);
        rec.extend_from_slice(&(body.len() as u16).to_be_bytes());
        rec.extend(body);
        if r.below(2) == 0 {
            // a plausible ServerHello header
            for (b, h) in rec[5..].iter_mut().zip([2u8, 0, 0, 80]) {
                *b = h;
            }
        }
        let step = 1 + r.below(50);
        for chunk in rec.chunks(step) {
            if c.feed(chunk).is_err() {
                break;
            }
        }
    }
}
