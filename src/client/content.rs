//! What a sent message may be besides text: a link with its preview, a forward, a live message.

use super::error::Result;
use super::handle::Client;
use crate::types::Request;
use crate::types::model::{ChatMessage, ChatRef, LinkInfo};

impl Client {
    /// Sends `msg` to `to` as the user's own, marked forwarded.
    pub fn forward(&self, msg: &ChatMessage, to: ChatRef) -> Result<()> {
        self.request(Request::Forward {
            from: msg.chat,
            item: msg.id,
            to,
        })
    }
    /// Starts a live message, seen as it is written; `update_live` sends the rest.
    pub fn send_live(&self, chat: ChatRef, text: &str) -> Result<()> {
        self.request(Request::SendLive {
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
    /// `text` with the preview of the link it has.
    pub fn send_link(&self, chat: ChatRef, text: &str, preview: LinkInfo) -> Result<()> {
        self.request(Request::SendLink {
            chat,
            text: text.to_owned(),
            preview,
            reply_to: None,
        })
    }
}
