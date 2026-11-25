//! A tagged request: handled as any other, with what becomes of what it sends told against its tag.

use crate::types::model::SendStatus;
use crate::types::{Event, Request};

use crate::engine::agent_port::AgentPort;
use crate::engine::error::{EngineError, Result};
use crate::engine::machine::Engine;
use crate::limits::check;

impl<A: AgentPort> Engine<A> {
    /// `Queued` or `Refused` goes out before the agent's events are handled, so it comes
    /// before any `Sent`.
    pub(super) fn handle_tagged(&mut self, tag: u64, request: Request) -> Result<()> {
        let start = self.out_len();
        self.tagging = Some(tag);
        let done = check(&request)
            .map_err(EngineError::from)
            .and_then(|()| self.route(request));
        self.tagging = None;
        let status = match &done {
            Ok(()) => SendStatus::Queued {
                item: self.own_item_after(start),
            },
            Err(e) => SendStatus::Refused {
                code: e.code,
                detail: e.detail.clone(),
            },
        };
        self.emit(Event::SendStatus {
            tag,
            member: None,
            status,
        });
        self.pump();
        done
    }
}
