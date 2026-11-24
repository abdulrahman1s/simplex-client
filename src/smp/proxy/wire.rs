//! The PKEY and PRES bodies (Protocol.hs `PKEY_`, `fwdResp`/`fwdRespP`; Transport.hs `CertChainPubKey`).
use crate::crypto::CbNonce;
use crate::encoding::{
    Decode, DecodeError, Decoder, Encode, EncodeError, Large, NonEmpty, VersionRange,
};

use crate::smp::responses::Response;
use crate::smp::version::FWD_NONCES;

/// `PKEY sessionId versionRange (CertChainPubKey chain signedKey)`.
pub(crate) fn decode_pkey(d: &mut Decoder<'_>) -> Result<Response, DecodeError> {
    Ok(Response::Pkey {
        session_id: Vec::decode(d)?,
        range: VersionRange::decode(d)?,
        chain: NonEmpty::<Large>::decode(d)?
            .into_vec()
            .into_iter()
            .map(|c| c.0)
            .collect(),
        signed_key: Large::decode(d)?.0,
    })
}

pub(crate) fn encode_pkey(
    out: &mut Vec<u8>,
    session_id: &[u8],
    range: &VersionRange,
    chain: &[Vec<u8>],
    signed_key: &[u8],
) -> Result<(), EncodeError> {
    session_id.encode(out)?;
    range.encode(out)?;
    let chain = chain.iter().cloned().map(Large).collect::<Vec<_>>();
    NonEmpty::new(chain)
        .ok_or(EncodeError::EmptyList)?
        .encode(out)?;
    Large(signed_key.to_vec()).encode(out)
}

/// From v23 the relay picks a random nonce for its answer and says so; before, it is the reversed corrId.
pub(crate) fn decode_pres(v: u16, d: &mut Decoder<'_>) -> Result<Response, DecodeError> {
    let nonce = if v >= FWD_NONCES {
        match d.byte()? {
            b'0' => None,
            b'1' => Some(CbNonce::new(d.take_array()?)),
            t => return Err(DecodeError::BadTag("Maybe CbNonce", t)),
        }
    } else {
        None
    };
    Ok(Response::Pres {
        nonce,
        body: d.take_all().to_vec(),
    })
}

pub(crate) fn encode_pres(v: u16, out: &mut Vec<u8>, nonce: Option<&CbNonce>, body: &[u8]) {
    if v >= FWD_NONCES {
        match nonce {
            None => out.push(b'0'),
            Some(n) => {
                out.push(b'1');
                out.extend_from_slice(n.as_bytes());
            }
        }
    }
    out.extend_from_slice(body);
}
