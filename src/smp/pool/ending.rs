//! How a link ends: waiting commands answered, Down or ConnectFailed told once, the next dial scheduled.
use std::time::Duration;

use crate::smp::error::SmpError;
use crate::smp::events::SmpEventKind;

use super::slot::Slot;
use super::why::{RATE_LIMIT_WAIT, Why, ms};

impl Slot {
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
    pub(super) fn after_session(
        &mut self,
        events: Vec<SmpEventKind>,
        now: u64,
        out: &mut Vec<SmpEventKind>,
    ) {
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
