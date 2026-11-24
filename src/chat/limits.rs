//! Size and count limits (Protocol.hs 7.1.4 `maxEncodedMsgLength` and friends).

/// The longest message as JSON, and the most a sent body may take: 15 602 bytes (Protocol.hs
/// `maxEncodedMsgLength`). A longer message is refused (`SELargeMsg`).
pub const MAX_ENCODED_MSG_LENGTH: usize = 15602;
/// The most a body to a post-quantum connection may take, compressed if need be: 13 380 bytes
/// (Protocol.hs `maxEncodedMsgLengthPQ`).
pub const MAX_ENCODED_MSG_LENGTH_PQ: usize = 13380;
/// A forwarded batch: 15 763 bytes (Protocol.hs `maxForwardBatchLength`).
pub const MAX_FORWARD_BATCH_LENGTH: usize = MAX_ENCODED_MSG_LENGTH + 161;
/// Conn-info (x.info, x.contact, ...) limits: `e2eEncConnInfoLength - 2 - 256`, without and
/// with PQ: 14 574 bytes (Internal.hs `compressConnInfo`).
pub const MAX_CONN_INFO_LENGTH: usize = 14574;
/// As [`MAX_CONN_INFO_LENGTH`], with PQ: 10 848 bytes.
pub const MAX_CONN_INFO_LENGTH_PQ: usize = 10848;
/// A profile message: 16 384 bytes (Protocol.hs `maxEncodedProfileMsgLength`).
pub const MAX_ENCODED_PROFILE_MSG_LENGTH: usize = 16384;
/// What a compressed body may grow to: 65 536 bytes (Protocol.hs `maxDecompressedMsgLength`).
pub const MAX_DECOMPRESSED_MSG_LENGTH: usize = 65536;
/// Messages in one batch: 255 (Protocol.hs `maxBatchElementCount`).
pub const MAX_BATCH_ELEMENT_COUNT: usize = 255;
/// How deep a forward may nest: 1 (Protocol.hs `maxFwdDepth`).
pub const MAX_FWD_DEPTH: usize = 1;
/// Bodies up to this length stay as they are inside the `X` container: 180 bytes (Compression.hs).
pub const MAX_LENGTH_PASSTHROUGH: usize = 180;
/// The zstd level bodies are compressed at: 3 (Compression.hs).
pub const COMPRESSION_LEVEL: i32 = 3;
