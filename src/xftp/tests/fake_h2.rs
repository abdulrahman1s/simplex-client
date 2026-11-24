//! The server half of HTTP/2 that XFTP needs: requests in, one response each, a WINDOW_UPDATE for every DATA frame.
use crate::h2::encode_headers;
use crate::h2::frames::{
    CLIENT_PREFACE, DATA, END_HEADERS, END_STREAM, FrameHeader, GOAWAY, HEADER_LEN, HEADERS,
    SETTINGS, encode_frame, encode_settings, encode_window_update, parse_header,
};
use crate::tls::TlsError;
use crate::tls::testing::ServerConn;

/// A request's stream and body.
pub struct Req {
    pub stream: u32,
    pub body: Vec<u8>,
}

fn frame(conn: &mut ServerConn) -> Result<(FrameHeader, Vec<u8>), TlsError> {
    let head: [u8; HEADER_LEN] = conn
        .recv_exact(HEADER_LEN)?
        .try_into()
        .map_err(|_| TlsError::Protocol("frame header"))?;
    let h = parse_header(&head);
    Ok((h, conn.recv_exact(h.length)?))
}

/// The preface and SETTINGS both ways.
pub fn accept(conn: &mut ServerConn) -> Result<(), TlsError> {
    if conn.recv_exact(CLIENT_PREFACE.len())? != CLIENT_PREFACE {
        return Err(TlsError::Protocol("no HTTP/2 preface"));
    }
    conn.send(&encode_settings(&[]))
}

/// The next whole request; `None` once the client sent GOAWAY.
pub fn request(conn: &mut ServerConn) -> Result<Option<Req>, TlsError> {
    let mut req: Option<Req> = None;
    loop {
        let (h, p) = frame(conn)?;
        match h.kind {
            HEADERS => {
                req = Some(Req {
                    stream: h.stream,
                    body: Vec::new(),
                });
            }
            DATA if !p.is_empty() => {
                let n = p.len() as u32;
                conn.send(
                    &[
                        encode_window_update(0, n),
                        encode_window_update(h.stream, n),
                    ]
                    .concat(),
                )?;
                if let Some(r) = req.as_mut() {
                    r.body.extend(p);
                }
            }
            GOAWAY => return Ok(None),
            _ => {}
        }
        if h.kind != SETTINGS && h.flags & END_STREAM != 0 && matches!(h.kind, HEADERS | DATA) {
            return Ok(req);
        }
    }
}

/// `200` with `body` in frames the client's default maximum allows.
pub fn respond(conn: &mut ServerConn, stream: u32, body: &[u8]) -> Result<(), TlsError> {
    let headers = encode_headers(&[(":status", "200")]);
    let end = if body.is_empty() { END_STREAM } else { 0 };
    let mut out = encode_frame(HEADERS, END_HEADERS | end, stream, &headers);
    let mut parts = body.chunks(16384).peekable();
    while let Some(part) = parts.next() {
        let last = if parts.peek().is_none() {
            END_STREAM
        } else {
            0
        };
        out.extend(encode_frame(DATA, last, stream, part));
    }
    conn.send(&out)
}
