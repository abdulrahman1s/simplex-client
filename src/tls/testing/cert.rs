//! Minimal DER certificates signed for real, so chain checks have something to check.
use crate::crypto::{Alg, KeyPair, PublicKey, encode_public_der};

fn der(tag: u8, parts: &[&[u8]]) -> Vec<u8> {
    let body = parts.concat();
    let mut out = vec![tag];
    match body.len() {
        n if n < 128 => out.push(n as u8),
        n if n < 256 => out.extend([0x81, n as u8]),
        n => out.extend([0x82, (n >> 8) as u8, n as u8]),
    }
    out.extend(body);
    out
}

fn signature_algorithm(alg: Alg) -> Vec<u8> {
    let oid = if alg == Alg::Ed448 { 0x71 } else { 0x70 };
    der(0x30, &[&der(0x06, &[&[0x2b, 0x65, oid]])])
}

/// Valid from 1970 to 9999, so fake clocks anywhere in between accept it.
pub(crate) const ALWAYS: (&str, &str) = ("19700101000000Z", "99991231235959Z");

/// The Validity SEQUENCE of two GeneralizedTimes.
pub(crate) fn validity((not_before, not_after): (&str, &str)) -> Vec<u8> {
    let time = |t: &str| der(0x18, &[t.as_bytes()]);
    der(0x30, &[&time(not_before), &time(not_after)])
}

/// A certificate for `subject`, signed by `issuer` (the same pair for a self-signed one).
pub(crate) fn make_cert(subject: &PublicKey, issuer: &KeyPair, serial: u8) -> Vec<u8> {
    make_cert_valid(subject, issuer, serial, ALWAYS)
}

/// `make_cert` with the given notBefore and notAfter.
pub(crate) fn make_cert_valid(
    subject: &PublicKey,
    issuer: &KeyPair,
    serial: u8,
    dates: (&str, &str),
) -> Vec<u8> {
    let alg = signature_algorithm(issuer.public.alg());
    let empty = der(0x30, &[]);
    let tbs = der(
        0x30,
        &[
            &der(0xa0, &[&der(0x02, &[&[2]])]),
            &der(0x02, &[&[serial]]),
            &alg,
            &empty,
            &validity(dates),
            &empty,
            &encode_public_der(subject.alg(), subject.as_bytes()),
        ],
    );
    let signature = issuer.private.sign(&tbs).map(|s| s.as_bytes().to_vec());
    let signature = signature.unwrap_or_default();
    der(0x30, &[&tbs, &alg, &der(0x03, &[&[0], &signature])])
}
