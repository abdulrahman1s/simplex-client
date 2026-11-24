//! How a session ends: a deadline passed, the connection failed, or the owner closed it.
use super::super::error::XftpError;
use super::machine::{Phase, SessionEvent, XftpSession};

impl XftpSession {
    /// The step under way failed if its deadline has passed.
    pub fn tick(&mut self, now: u64) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        if self.deadline.is_some_and(|d| now > d) {
            self.fail(XftpError::TimedOut, &mut events);
        }
        events
    }

    /// Ends the session from our side: close_notify goes out and a command in flight is
    /// answered `Cancelled`.
    pub fn close(&mut self) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        if self.is_down() {
            return events;
        }
        let goaway = self.h2.as_mut().and_then(|h2| {
            h2.close();
            h2.take_outgoing()
        });
        if let Some(Ok(records)) = goaway.map(|frames| self.tls.write_app(&frames)) {
            self.out.push_back(records);
        }
        if let Ok(records) = self.tls.close()
            && !records.is_empty()
        {
            self.out.push_back(records);
        }
        self.fail(XftpError::Cancelled, &mut events);
        events
    }

    /// The byte pipe under the session ended: the command in flight, if any, hears `why`.
    pub fn end(&mut self, why: XftpError) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        self.fail(why, &mut events);
        events
    }

    /// The session is down; the command in flight, if any, hears why.
    pub(super) fn fail(&mut self, why: XftpError, events: &mut Vec<SessionEvent>) {
        if self.is_down() {
            return;
        }
        self.phase = Phase::Down;
        self.deadline = None;
        if let Some(h2) = &mut self.h2 {
            h2.abandon();
        }
        let busy = std::mem::take(&mut self.busy);
        events.push(if busy {
            SessionEvent::Answer(Err(why))
        } else {
            SessionEvent::Closed(why)
        });
    }
}
