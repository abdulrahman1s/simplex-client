//! File and picture sizes (simplexmq Description.hs `maxFileSize`, Chunks.hs; Commands.hs `maxProfileImageSize`; the apps' `MAX_*` in Utils.kt and FileUtils.swift).

pub use crate::xftp::description::DESCR_PART_SIZE;
pub use crate::xftp::{
    CHUNK_SIZE_0, CHUNK_SIZE_1, CHUNK_SIZE_2, CHUNK_SIZE_3, MAX_FILE_SIZE, MAX_FILE_SIZE_HARD,
    SERVER_CHUNK_SIZES,
};

/// The longest `data:` URI a profile or group picture is: 12 500 characters (Commands.hs
/// `maxProfileImageSize`). A longer one is refused.
pub const MAX_PICTURE_URI: usize = 12_500;

/// The longest file sent inline over SMP: 8 000 000 bytes (apps `MAX_FILE_SIZE_SMP`). This
/// client sends every file over XFTP, so it is listed for reference.
pub const MAX_FILE_SIZE_SMP: u64 = 8_000_000;

/// The size the apps compress an image to before sending: 261 120 bytes (apps
/// `MAX_IMAGE_SIZE`). A target, not a refusal.
pub const MAX_IMAGE_SIZE: u64 = 261_120;

/// Images up to 522 240 bytes are received without asking (Commands.hs `maxImageSize`, apps
/// `MAX_IMAGE_SIZE_AUTO_RCV`); voice messages share the limit.
pub const MAX_IMAGE_SIZE_AUTO_RCV: u64 = 2 * MAX_IMAGE_SIZE;

/// Videos up to 1 047 552 bytes are received without asking (apps `MAX_VIDEO_SIZE_AUTO_RCV`).
pub const MAX_VIDEO_SIZE_AUTO_RCV: u64 = 1_047_552;

/// The size an image's preview is made to: 14 000 bytes (Commands.hs `maxImagePreviewSize`).
pub const MAX_IMAGE_PREVIEW_SIZE: usize = 14_000;

/// The longest voice message the apps record: 300 seconds (apps `MAX_VOICE_MESSAGE_LENGTH`).
pub const MAX_VOICE_SECONDS: u32 = 300;

/// Servers a chunk may name: 16 (Description.hs `validateFileDescription`).
pub const MAX_CHUNK_REPLICAS: usize = 16;

/// Chunk replicas a whole description may name: 131 072 (Description.hs `validateFileDescription`).
pub const MAX_FILE_REPLICAS: usize = 131_072;

/// The largest redirect file, the one holding a real description: one 4 MiB chunk
/// (Description.hs `validateFileDescription`).
pub const MAX_REDIRECT_SIZE: u64 = CHUNK_SIZE_3 as u64;

/// A description URI longer than 1 002 characters no longer fits a small QR code
/// (Description.hs `qrSizeLimit`).
pub const QR_SIZE_LIMIT: usize = 1002;
