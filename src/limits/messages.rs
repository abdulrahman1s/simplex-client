//! Message size, mentions, and the edit and delete windows (Protocol.hs `maxEncodedMsgLength`; Internal.hs `maxSndMentions`; Messages.hs `deletable'`).

pub use crate::chat::limits::{
    COMPRESSION_LEVEL, MAX_BATCH_ELEMENT_COUNT, MAX_CONN_INFO_LENGTH, MAX_CONN_INFO_LENGTH_PQ,
    MAX_DECOMPRESSED_MSG_LENGTH, MAX_ENCODED_MSG_LENGTH, MAX_ENCODED_MSG_LENGTH_PQ,
    MAX_ENCODED_PROFILE_MSG_LENGTH, MAX_FORWARD_BATCH_LENGTH, MAX_FWD_DEPTH,
    MAX_LENGTH_PASSTHROUGH,
};

/// The longest profile or group profile update as JSON: 14 694 bytes (Protocol.hs
/// `maxEncodedInfoLength`, checked by Commands.hs `checkInfoSize`).
pub const MAX_ENCODED_INFO_LENGTH: usize = 14_694;

/// Members one message may mention: 3 (Internal.hs `maxSndMentions`).
pub const MAX_SENT_MENTIONS: usize = 3;

/// Mentions read from a received message: the first 5 distinct ones (Internal.hs `maxRcvMentions`).
pub const MAX_RECEIVED_MENTIONS: usize = 5;

const DAY_MS: i64 = 24 * 3600 * 1000;

/// One's own message may be edited for a day after it was sent (Messages.hs `deletable'`
/// with `nominalDay`).
pub const EDIT_WINDOW_MS: i64 = DAY_MS;

/// One's own message may be deleted for everyone for a day and six hours (Commands.hs
/// `APIDeleteChatItem`: `nominalDay + 6 * 3600`).
pub const DELETE_WINDOW_MS: i64 = DAY_MS + 6 * 3600 * 1000;

/// The bytes `s` takes inside a JSON message, quotes left out (the apps' `chatJsonLength`).
#[must_use]
pub fn json_len(s: &str) -> usize {
    serde_json::to_string(s).map_or(s.len(), |j| j.len() - 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_len_counts_escapes_and_utf8_bytes() {
        assert_eq!(json_len("abc"), 3);
        assert_eq!(json_len("a\"b"), 4);
        assert_eq!(json_len("line\n"), 6);
        assert_eq!(json_len("ж"), 2);
    }
}
