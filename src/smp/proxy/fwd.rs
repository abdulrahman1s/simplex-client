//! What a proxy hands the destination relay inside RFWD, and what comes back in RRES (Protocol.hs `FwdTransmission`, `FwdResponse`).
use crate::encoding::{Decode, DecodeError, Decoder, Encode, EncodeError, Tail};

/// `FwdTransmission`: the client's PFWD, as the relay needs it to open the sealed command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FwdTransmission {
    /// The PFWD's corrId, which is the client's nonce.
    pub corr_id: Vec<u8>,
    pub version: u16,
    pub key: crate::crypto::PublicKey,
    pub body: Vec<u8>,
}

impl Encode for FwdTransmission {
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        (&self.corr_id, &self.version, &self.key).encode(out)?;
        out.extend_from_slice(&self.body);
        Ok(())
    }
}

impl Decode for FwdTransmission {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, DecodeError> {
        let (corr_id, version, key) = <(Vec<u8>, u16, crate::crypto::PublicKey)>::decode(d)?;
        if corr_id.len() != 24 {
            return Err(DecodeError::Invalid("CorrCbNonce"));
        }
        Ok(Self {
            corr_id,
            version,
            key,
            body: Tail::decode(d)?.0,
        })
    }
}

/// `FwdResponse`: the relay's sealed answer and the corrId it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FwdResponse {
    pub corr_id: Vec<u8>,
    pub body: Vec<u8>,
}

impl Encode for FwdResponse {
    fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.corr_id.encode(out)?;
        out.extend_from_slice(&self.body);
        Ok(())
    }
}

impl Decode for FwdResponse {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, DecodeError> {
        let corr_id = Vec::<u8>::decode(d)?;
        if corr_id.len() != 24 {
            return Err(DecodeError::Invalid("CorrCbNonce"));
        }
        Ok(Self {
            corr_id,
            body: Tail::decode(d)?.0,
        })
    }
}
