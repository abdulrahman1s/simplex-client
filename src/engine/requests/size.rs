//! A message as sent must fit (Store/Messages.hs `createNewSndMessage`: `SELargeMsg`; Internal.hs `compressToLimit`).

use crate::chat::{ChatError, ChatMessage, encode_chat_message};
use crate::engine::error::{Code, Result, fail};
use crate::limits::MAX_ENCODED_MSG_LENGTH;

/// The message's JSON fits before compression, as upstream measures it when the message is made.
pub(crate) fn sized(msg: &ChatMessage) -> Result<()> {
    if msg.to_json()?.len() > MAX_ENCODED_MSG_LENGTH {
        return Err(fail(Code::TextTooLong, "message too large"));
    }
    Ok(())
}

/// The body for the agent, once `sized`; one that does not compress to the limit is too long too.
pub(crate) fn encode_sized(msg: &ChatMessage, pq: bool) -> Result<Vec<u8>> {
    sized(msg)?;
    encode_chat_message(msg, pq).map_err(|e| match e {
        ChatError::TooLarge => fail(Code::TextTooLong, "large compressed body"),
        e @ (ChatError::Parse(_)
        | ChatError::Limit(_)
        | ChatError::Compression(_)
        | ChatError::Random) => e.into(),
    })
}
