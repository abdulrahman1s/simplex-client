//! Application data heard: a content type the client registered becomes `Event::Data`, not a message.

use serde_json::{Map, Value};

use crate::chat::app_data::data_of;
use crate::chat::container::MsgContainer;
use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// The tag and data of `c` when its content type is registered. One with a file stays a
    /// message, so the file can still be received.
    pub(crate) fn data_in(&self, c: &MsgContainer) -> Option<(String, Map<String, Value>)> {
        if c.file.is_some() {
            return None;
        }
        let (tag, json) = data_of(&c.content)?;
        self.data_tags.contains(tag).then(|| (tag.to_owned(), json))
    }
}
