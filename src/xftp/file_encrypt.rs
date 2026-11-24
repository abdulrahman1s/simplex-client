//! Encrypting a file for upload: `Int64 length | header | content | '#' padding | tag` under one secretbox stream (Crypto.hs `encryptFile`).
#[cfg(feature = "native")]
use std::fs::File;
use std::io::{BufWriter, Read, Write};
#[cfg(feature = "native")]
use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::crypto::{CbNonce, SbKey, SbState};

use super::error::XftpError;
use super::progress::CancelFlag;
use super::version::AUTH_TAG_SIZE;

const BLOCK: usize = 64 * 1024;
const FILL: u8 = b'#';

struct Sealer<W: Write> {
    state: SbState,
    out: BufWriter<W>,
    written: u64,
}

impl<W: Write> Sealer<W> {
    fn put(&mut self, buf: &mut [u8]) -> Result<(), XftpError> {
        self.state.encrypt(buf);
        self.out.write_all(buf)?;
        self.written += buf.len() as u64;
        Ok(())
    }
}

#[cfg(feature = "native")]
pub struct EncryptParams<'a> {
    pub src: &'a Path,
    /// The encoded `FileHeader`.
    pub header: &'a [u8],
    pub key: &'a SbKey,
    pub nonce: &'a CbNonce,
    /// The length of `src`.
    pub content_len: u64,
    /// The size of the output, the sum of its chunks.
    pub enc_size: u64,
    pub dst: &'a Path,
}

/// Writes the encrypted file to `p.dst`: exactly `p.enc_size` bytes. `progress` gets the plaintext bytes read so far.
#[cfg(feature = "native")]
pub fn encrypt_file(
    p: &EncryptParams<'_>,
    cancel: CancelFlag<'_>,
    progress: &mut dyn FnMut(u64),
) -> Result<(), XftpError> {
    let layout = Layout {
        header: p.header,
        key: p.key,
        nonce: p.nonce,
        content_len: p.content_len,
        enc_size: p.enc_size,
    };
    layout.pad_len()?;
    let out = File::create(p.dst)?;
    seal(&layout, File::open(p.src)?, out, cancel, progress)
}

/// [`encrypt_file`] in memory, for callers without a file system: `enc_size` bytes.
pub fn encrypt_bytes(
    content: &[u8],
    header: &[u8],
    key: &SbKey,
    nonce: &CbNonce,
    enc_size: u64,
) -> Result<Vec<u8>, XftpError> {
    let layout = Layout {
        header,
        key,
        nonce,
        content_len: content.len() as u64,
        enc_size,
    };
    let mut out = Vec::with_capacity(usize::try_from(enc_size).unwrap_or(0));
    let cancel = AtomicBool::new(false);
    seal(&layout, content, &mut out, CancelFlag(&cancel), &mut |_| {})?;
    Ok(out)
}

struct Layout<'a> {
    header: &'a [u8],
    key: &'a SbKey,
    nonce: &'a CbNonce,
    content_len: u64,
    enc_size: u64,
}

impl Layout<'_> {
    fn pad_len(&self) -> Result<u64, XftpError> {
        let file_size = self.header.len() as u64 + self.content_len;
        self.enc_size
            .checked_sub(AUTH_TAG_SIZE as u64 + 8 + file_size)
            .ok_or_else(|| XftpError::File("encrypted size is too small for the file".into()))
    }
}

fn seal(
    layout: &Layout<'_>,
    mut reader: impl Read,
    out: impl Write,
    cancel: CancelFlag<'_>,
    progress: &mut dyn FnMut(u64),
) -> Result<(), XftpError> {
    let Layout {
        header,
        key,
        nonce,
        content_len,
        enc_size,
    } = *layout;
    let file_size = header.len() as u64 + content_len;
    let pad_len = layout.pad_len()?;
    let mut sealer = Sealer {
        state: SbState::new(key, nonce),
        out: BufWriter::new(out),
        written: 0,
    };
    let mut first = [
        &i64::try_from(file_size)
            .map_err(|_| XftpError::File("file too large".into()))?
            .to_be_bytes()[..],
        header,
    ]
    .concat();
    sealer.put(&mut first)?;
    let mut buf = vec![0u8; BLOCK];
    let mut left = content_len;
    while left > 0 {
        cancel.check()?;
        let n = usize::try_from(left).map_or(BLOCK, |l| l.min(BLOCK));
        reader
            .read_exact(&mut buf[..n])
            .map_err(|_| XftpError::File("the file changed while it was encrypted".into()))?;
        sealer.put(&mut buf[..n])?;
        left -= n as u64;
        progress(content_len - left);
    }
    let mut left = pad_len;
    while left > 0 {
        let n = usize::try_from(left).map_or(BLOCK, |l| l.min(BLOCK));
        buf[..n].fill(FILL);
        sealer.put(&mut buf[..n])?;
        left -= n as u64;
    }
    let Sealer {
        state,
        mut out,
        written,
    } = sealer;
    out.write_all(&state.tag())?;
    out.flush()?;
    debug_assert_eq!(written + AUTH_TAG_SIZE as u64, enc_size);
    Ok(())
}
