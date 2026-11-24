//! One relay in the pool: its link while there is one, and when to dial again after it ends.
use std::sync::Arc;

use crate::smp::address::SmpServer;
use crate::smp::config::ConnConfig;
use crate::smp::error::SmpError;
use crate::smp::events::SmpEventKind;
use crate::smp::flow::Backoff;
use crate::smp::tunnel::parse_tunnel_reply;

use super::link::Link;
use super::pipe::PipeInput;
use super::why::{Why, refused};

pub(super) struct Slot {
    pub(super) server: Arc<SmpServer>,
    pub(super) link: Option<Link>,
    /// While there is no link: when to dial again.
    pub(super) retry_at: u64,
    pub(super) backoff: Backoff,
    /// A Down went out since the last Up, so another failure tells the owner nothing new.
    pub(super) down_told: bool,
}

impl Slot {
    pub(super) fn new(server: SmpServer, cfg: &ConnConfig, now: u64) -> Self {
        Self {
            server: Arc::new(server),
            link: None,
            retry_at: now,
            backoff: Backoff::new(cfg.reconnect_min, cfg.reconnect_max),
            down_told: false,
        }
    }

    pub(super) fn on_input(
        &mut self,
        input: PipeInput,
        now: u64,
        token: Option<&str>,
        out: &mut Vec<SmpEventKind>,
        notes: &mut Vec<String>,
    ) {
        let Some(link) = &mut self.link else { return };
        let done = match input {
            PipeInput::Open if !link.opened => {
                link.on_open(&self.server, token).map_err(Why::Failed)
            }
            PipeInput::Open => Ok(()),
            PipeInput::Text(text) => match parse_tunnel_reply(&text) {
                Ok(()) => link.send_client_hello().map_err(Why::Failed),
                Err(e) => Err(refused(&e, notes)),
            },
            PipeInput::Binary(_) if !link.is_through() => Err(Why::Failed(SmpError::Unexpected(
                "bytes before the tunnel opened",
            ))),
            PipeInput::Binary(bytes) => {
                let events = link.session.feed(&bytes, now);
                self.after_session(events, now, out);
                Ok(())
            }
            PipeInput::Closed { code, reason } if code == 1000 && reason == "lifetime" => {
                Err(Why::Lifetime)
            }
            PipeInput::Closed { code, reason } => Err(Why::Failed(SmpError::Io(format!(
                "the tunnel closed ({code} {reason})"
            )))),
        };
        if let Err(why) = done {
            self.end(why, now, out);
        }
    }

    pub(super) fn tick(&mut self, now: u64, out: &mut Vec<SmpEventKind>) {
        if let Some(link) = &mut self.link {
            let events = link.session.tick(now);
            self.after_session(events, now, out);
        }
    }

    pub(super) fn flush(&mut self, now: u64, out: &mut Vec<SmpEventKind>) {
        if let Some(Err(e)) = self.link.as_mut().map(Link::flush) {
            self.end(Why::Failed(e), now, out);
        }
    }
}
