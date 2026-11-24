//! The end of an in-memory download: size, digest and tag checked, a redirect followed once (Agent.hs `decryptFile`, redirect handling).
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
        if !self.outer {
            return Ok(Step::Done(Fetched {
                file_name: header.file_name,
                bytes,
            }));
        }
        let inner = self.inner(&bytes)?;
        *self = Self::new(&inner, self.limit)?;
        self.next_step()
    }

    /// The description a redirect holds; its size and digest are what the redirect promised.
    fn inner(&self, bytes: &[u8]) -> Result<FileDescription, XftpError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| XftpError::Description("redirected description is not text".into()))?;
        let inner = FileDescription::parse_valid(text).map_err(XftpError::Description)?;
        let promised = self.fd.redirect.as_ref();
        let same = promised.is_some_and(|r| r.size == inner.size && r.digest == inner.digest);
        if inner.redirect.is_some() || !same {
            return Err(XftpError::Mismatch("redirected description"));
        }
        Ok(inner)
    }
}
