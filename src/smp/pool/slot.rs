//! One relay in the pool: its link while there is one, and when to dial again after it ends.
use std::sync::Arc;
use std::time::Duration;

use crate::smp::address::SmpServer;
use crate::smp::config::ConnConfig;
use crate::smp::error::SmpError;
use crate::smp::events::SmpEventKind;
use crate::smp::flow::Backoff;
use crate::smp::tunnel::parse_tunnel_reply;

use super::link::Link;
use super::pipe::PipeInput;
use super::why::{RATE_LIMIT_WAIT, Why, ms, refused};

pub(super) struct Slot {
    pub(super) server: Arc<SmpServer>,
    pub(super) link: Option<Link>,
    /// While there is no link: when to dial again.
    pub(super) retry_at: u64,
    backoff: Backoff,
    /// A Down went out since the last Up, so another failure tells the owner nothing new.
    down_told: bool,
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
            PipeInput::Open if !link.opened => link
                .send_tunnel_hello(&self.server, token)
                .map_err(Why::Failed),
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

    /// Ends the link for a reason the session did not see itself: every waiting command is
    /// answered, then the owner hears Down (it was up) or ConnectFailed.
    pub(super) fn end(&mut self, why: Why, now: u64, out: &mut Vec<SmpEventKind>) {
        let Some(mut link) = self.link.take() else {
            return;
        };
        link.pipe.close();
        let was_up = link.session.is_up();
        out.extend(link.session.close());
        let err = match &why {
            Why::Failed(e) | Why::Limited(e) => e.clone(),
            Why::Lifetime => SmpError::Closed,
        };
        if was_up {
            out.push(SmpEventKind::Down(err));
            self.down_told = true;
        } else {
            out.push(SmpEventKind::ConnectFailed(err.clone()));
            self.tell_down(err, out);
        }
        let wait = match why {
            Why::Lifetime => Duration::ZERO,
            Why::Limited(_) => self.backoff.next_delay().max(RATE_LIMIT_WAIT),
            Why::Failed(_) => self.backoff.next_delay(),
        };
        self.retry_at = now.saturating_add(ms(wait));
    }

    /// A dial that failed before any link existed.
    pub(super) fn dial_failed(&mut self, err: SmpError, now: u64, out: &mut Vec<SmpEventKind>) {
        out.push(SmpEventKind::ConnectFailed(err.clone()));
        self.tell_down(err, out);
        self.retry_at = now.saturating_add(ms(self.backoff.next_delay()));
    }

    /// Passes the session's events on; when they end it, the pipe goes and a dial is scheduled.
    fn after_session(&mut self, events: Vec<SmpEventKind>, now: u64, out: &mut Vec<SmpEventKind>) {
        let mut ended = None;
        for e in &events {
            match e {
                SmpEventKind::Up(_) => {
                    self.backoff.reset();
                    self.down_told = false;
                }
                SmpEventKind::Down(_) => {
                    self.down_told = true;
                    ended = Some(None);
                }
                SmpEventKind::ConnectFailed(err) => ended = Some(Some(err.clone())),
                SmpEventKind::Msg { .. }
                | SmpEventKind::End { .. }
                | SmpEventKind::Deld { .. }
                | SmpEventKind::Reply { .. }
                | SmpEventKind::Unexpected { .. } => {}
            }
        }
        out.extend(events);
        let Some(failed) = ended else { return };
        if let Some(mut link) = self.link.take() {
            link.pipe.close();
        }
        if let Some(err) = failed {
            self.tell_down(err, out);
        }
        self.retry_at = now.saturating_add(ms(self.backoff.next_delay()));
    }

    /// The first failure since the last Up also says Down, so the owner learns the relay is
    /// unreachable even when it was never up.
    fn tell_down(&mut self, err: SmpError, out: &mut Vec<SmpEventKind>) {
        if !self.down_told {
            out.push(SmpEventKind::Down(err));
            self.down_told = true;
        }
    }
}
