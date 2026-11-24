//! The end of an in-memory download: size, digest and tag checked (Agent.hs `decryptFile`).
use subtle::ConstantTimeEq;

use crate::crypto::{CbNonce, SbKey, sha512};

use super::super::description::{FileDescription, Party};
use super::super::error::XftpError;
use super::super::file_decrypt::decrypt_bytes;
use super::download::{Fetched, MemDownload};
use super::step::Step;

/// What a description must be before anything is fetched for it.
pub(super) fn check(fd: &FileDescription, limit: u64) -> Result<(), XftpError> {
    if fd.party != Party::Recipient {
        return Err(XftpError::Description(
            "a recipient description is needed to download".into(),
        ));
    }
    fd.validate().map_err(XftpError::Description)?;
    // Internal.hs `receiveViaCompleteFD`: 7.1.4 refuses redirects until it checks their relays.
    if fd.redirect.is_some() {
        return Err(XftpError::Description("redirect not allowed".into()));
    }
    SbKey::from_slice(&fd.key)?;
    CbNonce::from_slice(&fd.nonce)?;
    if fd.digest.len() != 64 {
        return Err(XftpError::Description(
            "the file digest must be 64 bytes".into(),
        ));
    }
    if fd.size > limit {
        return Err(XftpError::File(format!(
            "the file is larger than the {limit} bytes allowed in memory"
        )));
    }
    Ok(())
}

impl MemDownload {
    pub(super) fn finish(&mut self) -> Result<Step<Fetched>, XftpError> {
        if self.fetched.len() as u64 != self.fd.size {
            return Err(XftpError::Mismatch("file size"));
        }
        if !bool::from(sha512(&self.fetched).as_slice().ct_eq(&self.fd.digest)) {
            return Err(XftpError::Mismatch("file digest"));
        }
        let (key, nonce) = (
            SbKey::from_slice(&self.fd.key)?,
            CbNonce::from_slice(&self.fd.nonce)?,
        );
        let (header, bytes) = decrypt_bytes(&std::mem::take(&mut self.fetched), &key, &nonce)?;
        Ok(Step::Done(Fetched {
            file_name: header.file_name,
            bytes,
        }))
    }
}
