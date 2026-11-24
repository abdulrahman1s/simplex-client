//! The mock server's ServerHello, and plaintext records to carry it.
use super::options::MockOpts;
use crate::crypto::KeyPair;
use crate::tls::record::HANDSHAKE;
use crate::tls::wire::{extension, handshake_message, put_u16};

const HELLO_RETRY_REQUEST_RANDOM: [u8; 32] = [
    0xcf, 0x21, 0xad, 0x74, 0xe5, 0x9a, 0x61, 0x11, 0xbe, 0x1d, 0x8c, 0x02, 0x1e, 0x65, 0xb8, 0x91,
    0xc2, 0xa2, 0x11, 0x16, 0x7a, 0xbb, 0x8c, 0x5e, 0x07, 0x9e, 0x09, 0xe2, 0xc8, 0xa8, 0x33, 0x9c,
];

/// The ServerHello message for `eph`, with the faults `o` asks for.
pub(super) fn server_hello(o: &MockOpts, eph: &KeyPair, random: [u8; 32]) -> Vec<u8> {
    let random = if o.hrr {
        HELLO_RETRY_REQUEST_RANDOM
    } else {
        random
    };
    let mut share = 0x001du16.to_be_bytes().to_vec();
    put_u16(&mut share, 32);
    share.extend_from_slice(eph.public.as_bytes());
    let exts = [extension(43, &[3, 4]), extension(51, &share)].concat();
    let mut body = vec![3, 3];
    body.extend_from_slice(&random);
    body.extend_from_slice(&[0, 0x13, 0x03, 0]);
    put_u16(&mut body, exts.len());
    body.extend(exts);
    handshake_message(2, &body)
}

/// One unprotected handshake record.
pub(super) fn plain_record(content: &[u8]) -> Vec<u8> {
    let mut record = vec![HANDSHAKE, 3, 3];
    put_u16(&mut record, content.len());
    record.extend_from_slice(content);
    record
}
