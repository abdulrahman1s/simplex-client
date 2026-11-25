//! Application data as a content type the official apps do not know, shown there as its text (Protocol.hs `MCUnknown`, `MsgContentTag`).
use serde_json::{Map, Value};

use super::content::MsgContent;

/// The tags `MsgContentTag` parses as its own; any other is `MCUnknown`.
pub const KNOWN_TAGS: [&str; 8] = [
    "text", "link", "image", "video", "voice", "file", "report", "chat",
];

/// Why a tag or its data cannot go as application data; `None` when they can.
#[must_use]
pub fn refusal(tag: &str, json: &Map<String, Value>) -> Option<&'static str> {
    if tag.is_empty() || tag.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("a data tag is not empty and has no spaces");
    }
    if KNOWN_TAGS.contains(&tag) {
        return Some("a data tag is not one of the message content types");
    }
    if json.contains_key("type") || json.contains_key("text") {
        return Some("data carries no \"type\" or \"text\": they hold the tag and the fallback");
    }
    None
}

/// `json` with `type` = `tag` and `text` = `fallback`: what `MCUnknown` sends as it is, and
/// what an official app shows as its text.
#[must_use]
pub fn content(tag: &str, json: &Map<String, Value>, fallback: &str) -> MsgContent {
    let mut o = json.clone();
    o.insert("type".to_owned(), Value::String(tag.to_owned()));
    o.insert("text".to_owned(), Value::String(fallback.to_owned()));
    MsgContent::Unknown {
        tag: tag.to_owned(),
        text: fallback.to_owned(),
        json: o,
    }
}

/// The tag and the data of an unknown content, without its `type` and `text`.
#[must_use]
pub fn data_of(c: &MsgContent) -> Option<(&str, Map<String, Value>)> {
    let MsgContent::Unknown { tag, json, .. } = c else {
        return None;
    };
    let mut o = json.clone();
    o.remove("type");
    o.remove("text");
    Some((tag, o))
}
