//! Where timed items are indexed in the namespace "chat": due ones by delete time, unread ones by chat.

use crate::types::model::ChatRef;

/// A timed item's delete time, so the due ones read back in order (`engine::timed`). Eviction:
/// when the item goes, by its timer or any other deletion, or with its chat.
#[must_use]
pub fn timed_due(at: i64, chat: ChatRef, item: i64) -> Vec<u8> {
    format!("tdue/{:020}/{}/{item}", at.max(0), chat_tag(chat)).into_bytes()
}

/// Every timed item's delete time, read back at start.
pub const TIMED_DUE_PREFIX: &[u8] = b"tdue/";

/// A received timed item not read yet: reading it starts its count. Eviction: when it is read
/// or goes, or with its chat.
#[must_use]
pub fn timed_unread(chat: ChatRef, item: i64) -> Vec<u8> {
    format!("tread/{}/{item}", chat_tag(chat)).into_bytes()
}

/// Every unread timed item, read back at start.
pub const TIMED_UNREAD_PREFIX: &[u8] = b"tread/";

/// `timed_due` back to (delete time, chat, item).
#[must_use]
pub fn parse_timed_due(key: &[u8]) -> Option<(i64, ChatRef, i64)> {
    let s = std::str::from_utf8(key).ok()?.strip_prefix("tdue/")?;
    let mut parts = s.split('/');
    let at = parts.next()?.parse().ok()?;
    let chat = parse_chat_tag(parts.next()?)?;
    let item = parts.next()?.parse().ok()?;
    Some((at, chat, item))
}

/// `timed_unread` back to (chat, item).
#[must_use]
pub fn parse_timed_unread(key: &[u8]) -> Option<(ChatRef, i64)> {
    let s = std::str::from_utf8(key).ok()?.strip_prefix("tread/")?;
    let (chat, item) = s.split_once('/')?;
    Some((parse_chat_tag(chat)?, item.parse().ok()?))
}

fn chat_tag(chat: ChatRef) -> String {
    match chat {
        ChatRef::Direct(f) => format!("d{f}"),
        ChatRef::Group(g) => format!("g{g}"),
    }
}

fn parse_chat_tag(tag: &str) -> Option<ChatRef> {
    if let Some(f) = tag.strip_prefix('d') {
        return f.parse().ok().map(ChatRef::Direct);
    }
    tag.strip_prefix('g')?.parse().ok().map(ChatRef::Group)
}
