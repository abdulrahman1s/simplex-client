//! One zstd frame: encode at level 3 with the content size, decode under strict bounds (spike zstd, Compression.hs).
use super::error::{ChatError, Result};
#[cfg(feature = "native")]
use super::limits::COMPRESSION_LEVEL;
use std::io::Read;

const MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];
/// A frame of at most 64 KiB needs no more window than this (ruzstd's default is 128 MiB).
const MAX_WINDOW: u64 = 1 << 20;

fn err(s: impl Into<String>) -> ChatError {
    ChatError::Compression(s.into())
}

/// `Frame_Content_Size` of the first frame, as `ZSTD_getFrameContentSize` reports it; `None` when the header omits it.
pub fn content_size(frame: &[u8]) -> Option<u64> {
    if frame.len() < 6 || frame[..4] != MAGIC {
        return None;
    }
    let d = frame[4];
    let (fcs_flag, single, dict) = (d >> 6, d >> 5 & 1 == 1, d & 3);
    let at = 5 + usize::from(!single) + [0, 1, 2, 4][usize::from(dict)];
    let n = match (fcs_flag, single) {
        (0, false) => return None,
        (0, true) => 1,
        (1, _) => 2,
        (2, _) => 4,
        _ => 8,
    };
    let mut v = [0u8; 8];
    v[..n].copy_from_slice(frame.get(at..at + n)?);
    let v = u64::from_le_bytes(v);
    Some(if n == 2 { v + 256 } else { v })
}

/// libzstd level 3; `bulk::compress` states the content size in the frame header.
#[cfg(feature = "native")]
pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    zstd::bulk::compress(data, COMPRESSION_LEVEL).map_err(|e| err(e.to_string()))
}

/// libzstd is C and does not build for wasm32-unknown-unknown.
#[cfg(not(feature = "native"))]
pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    compress_pure(data)
}

/// ruzstd's encoder (close to level 1), with the content size written into the header,
/// because ruzstd leaves it out and the decoder here and simplex-chat's both require it.
pub fn compress_pure(data: &[u8]) -> Result<Vec<u8>> {
    let size = u32::try_from(data.len()).map_err(|_| err("too large to compress"))?;
    let mut frame =
        ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest);
    // Expect magic, then a descriptor with no size, no single segment and no dictionary, then the window byte.
    let descriptor = match frame.get(..6) {
        Some(head) if head[..4] == MAGIC && head[4] & 0b1110_0011 == 0 => head[4],
        _ => return Err(err("unexpected ruzstd frame header")),
    };
    // Size flag 2: a four-byte `Frame_Content_Size` after the window descriptor.
    frame[4] = descriptor | 0b1000_0000;
    frame.splice(6..6, size.to_le_bytes());
    Ok(frame)
}

/// Decodes one frame that states its size, never producing more than `limit` bytes.
pub fn decompress_limited(frame: &[u8], limit: usize) -> Result<Vec<u8>> {
    let size = content_size(frame).ok_or_else(|| err("compressed size not specified"))?;
    if size > limit as u64 {
        return Err(ChatError::Limit("decompressed size exceeds limit"));
    }
    let mut fd = ruzstd::decoding::FrameDecoder::new();
    fd.set_max_window_size(MAX_WINDOW);
    let mut src = frame;
    let mut dec = ruzstd::decoding::StreamingDecoder::new_with_decoder(&mut src, &mut fd)
        .map_err(|e| err(e.to_string()))?;
    let mut out = Vec::with_capacity(usize::try_from(size).map_err(|_| err("size"))?);
    // `take(size + 1)` catches a frame that lies about its size and stops it growing past that.
    (&mut dec)
        .take(size + 1)
        .read_to_end(&mut out)
        .map_err(|e| err(e.to_string()))?;
    if out.len() as u64 != size {
        return Err(err(format!(
            "frame holds {} bytes, header says {size}",
            out.len()
        )));
    }
    Ok(out)
}
