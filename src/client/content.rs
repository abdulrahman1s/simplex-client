//! What a sent message may be besides text: a link with its preview, a forward, a live message, application data.

use super::error::Result;
use super::handle::Client;
use super::sending::SendHandle;
use crate::types::Request;
use crate::types::model::{ChatMessage, ChatRef, LinkInfo};

impl Client {
    /// Sends `msg` to `to` as the user's own, marked forwarded.
    pub fn forward(&self, msg: &ChatMessage, to: ChatRef) -> Result<SendHandle> {
        self.tagged(Request::Forward {
            from: msg.chat,
            item: msg.id,
            to,
        })
    }
    /// Starts a live message, seen as it is written; `update_live` sends the rest.
    pub fn send_live(&self, chat: ChatRef, text: &str) -> Result<SendHandle> {
        self.tagged(Request::SendLive {
            chat,
            text: text.to_owned(),
        })
    }
    /// The live message's text so far; `done` finishes it.
    pub fn update_live(&self, msg: &ChatMessage, text: &str, done: bool) -> Result<()> {
        self.request(Request::UpdateLive {
            chat: msg.chat,
            item: msg.id,
            text: text.to_owned(),
            done,
        })
    }
    /// Application data for programs as the content type `tag`, which the official apps show
    /// as `fallback_text`. Nothing is kept or shown here; a client that registered `tag`
    /// ([`ClientBuilder::data_tag`](super::ClientBuilder::data_tag)) gets it as
    /// [`Event::Data`](super::Event::Data). `json` may not hold `type` or `text`.
    ///
    /// In a channel it goes as the channel. Its relays replay only posts with text or a file
    /// to later subscribers (7.1.4 Internal.hs `processContentItem`): with an empty
    /// `fallback_text` the post reaches who is subscribed now and is never replayed. 7.1.4
    /// sends such posts too, so it is not refused.
    pub fn send_data(
        &self,
        chat: ChatRef,
        tag: &str,
        json: serde_json::Map<String, serde_json::Value>,
        fallback_text: &str,
    ) -> Result<SendHandle> {
        self.tagged(Request::SendData {
            chat,
            tag: tag.to_owned(),
            json,
            fallback_text: fallback_text.to_owned(),
        })
    }
    /// `text` with the preview of the link it has.
    pub fn send_link(&self, chat: ChatRef, text: &str, preview: LinkInfo) -> Result<SendHandle> {
        self.tagged(Request::SendLink {
            chat,
            text: text.to_owned(),
            preview,
            reply_to: None,
        })
    }
}
