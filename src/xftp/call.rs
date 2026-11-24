//! One command as an HTTP/2 POST and its answer, whatever drives the connection (Client.hs `sendXFTPCommand`, `sendXFTPTransmission`).
use crate::crypto::PrivateKey;
use crate::h2;

use super::commands::Command;
use super::error::XftpError;
use super::handshake::Negotiated;
use super::responses::Response;
use super::transmission::{decode_transmission, encode_transmission};
use super::version::BLOCK_SIZE;

/// The POST body: the command's padded block, then `file`.
pub fn encode_call(
    session: &Negotiated,
    key: Option<&PrivateKey>,
    entity: &[u8],
    cmd: &Command,
    file: &[u8],
) -> Result<Vec<u8>, XftpError> {
    let block = encode_transmission(
        &session.session_id,
        entity,
        &cmd.encode(session.version)?,
        key.filter(|_| cmd.is_authorized()),
    )?;
    Ok([block.as_slice(), file].concat())
}

/// The answer and the bytes after the response block. A server `ERR` is an `Err`.
pub fn decode_answer(
    session: &Negotiated,
    mut resp: h2::Response,
) -> Result<(Response, Vec<u8>), XftpError> {
    if resp.status != 200 {
        return Err(XftpError::Status(resp.status));
    }
    if resp.body.len() < BLOCK_SIZE {
        return Err(XftpError::Unexpected("response shorter than a block"));
    }
    let rest = resp.body.split_off(BLOCK_SIZE);
    let (_entity, command) = decode_transmission(&session.session_id, &resp.body)?;
    match Response::decode(session.version, &command)? {
        Response::Err(e) => Err(XftpError::Server(e)),
        r @ (Response::SndIds { .. }
        | Response::RcvIds { .. }
        | Response::File { .. }
        | Response::Ok
        | Response::Pong) => Ok((r, rest)),
    }
}

/// The POST every XFTP request is.
#[must_use]
pub fn post<'a>(authority: &'a str, body: &'a [u8]) -> h2::Request<'a> {
    h2::Request {
        method: "POST",
        path: "/",
        authority,
        headers: &[],
        body,
    }
}
