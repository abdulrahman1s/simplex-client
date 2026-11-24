//! One SMP session driven by pushed bytes and passed-in time: no threads, no clock, no socket (Client.hs `ProtocolClient`).
use std::collections::VecDeque;
use std::time::Duration;

use crate::smp::address::SmpServer;
use crate::smp::config::ConnConfig;
use crate::smp::error::SmpError;
use crate::smp::events::{SessionInfo, SmpEventKind};
use crate::smp::flow::{Liveness, Request};
use crate::smp::transport::Established;

use super::link::Link;
use super::outbox::{Outbox, Waiter};

pub(super) fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

pub(super) struct Live {
    pub(super) session: Established,
    pub(super) liveness: Liveness,
    pub(super) next_ping: Option<u64>,
}

pub(super) enum State {
    /// TLS and the SMP handshake, which must finish by `deadline`.
    Connecting {
        deadline: u64,
    },
    Up(Box<Live>),
    Down,
}

/// A session never reconnects: after `Down` or `ConnectFailed` the owner starts a new one,
/// waiting as a `Backoff` says.
pub struct SmpSession {
    pub(super) cfg: ConnConfig,
    pub(super) key_hash: Vec<u8>,
    pub(super) link: Link,
    pub(super) state: State,
    /// Commands sent before `Up`, with their tags: at most `cfg.queue_capacity`, then `Busy`.
    pub(super) queued: VecDeque<(Request, u64)>,
    pub(super) outbox: Outbox,
    /// A failure met while packing in `take_outgoing`; the next `tick` or `feed` reports it.
    pub(super) broken: Option<SmpError>,
}

impl SmpSession {
    /// The session and the TLS ClientHello to send once the byte pipe to `server` is open.
    pub fn start(
        server: &SmpServer,
        cfg: &ConnConfig,
        now_ms: u64,
    ) -> Result<(Self, Vec<u8>), SmpError> {
        let mut link = Link::start()?;
        let hello = link.take_outgoing().unwrap_or_default();
        let session = Self {
            key_hash: server.key_hash.clone(),
            link,
            state: State::Connecting {
                deadline: now_ms.saturating_add(ms(cfg.connect_timeout)),
            },
            queued: VecDeque::new(),
            outbox: Outbox::new(cfg.max_pending),
            broken: None,
            cfg: cfg.clone(),
        };
        Ok((session, hello))
    }

    #[must_use]
    pub fn is_up(&self) -> bool {
        matches!(self.state, State::Up(_))
    }

    #[must_use]
    pub fn session(&self) -> Option<SessionInfo> {
        match &self.state {
            State::Up(live) => Some(info(&live.session)),
            State::Connecting { .. } | State::Down => None,
        }
    }

    /// Queues `request`; its answer comes back as `SmpEventKind::Reply { tag, .. }` exactly once,
    /// with an error when the session ends first. Before `Up` it waits for the handshake.
    pub fn send(&mut self, request: Request, tag: u64, now_ms: u64) -> Result<(), SmpError> {
        match &mut self.state {
            State::Down => Err(SmpError::NotConnected),
            State::Connecting { .. } if self.queued.len() >= self.cfg.queue_capacity.max(1) => {
                Err(SmpError::Busy)
            }
            State::Connecting { .. } => {
                self.queued.push_back((request, tag));
                Ok(())
            }
            State::Up(live) => {
                let timeout = ms(self.cfg.command_timeout);
                let waiter = Waiter::Tagged(tag);
                (self.outbox).prepare(&live.session, &request, waiter, now_ms, timeout)
            }
        }
    }

    /// The next bytes for the socket; commands sent since the last call are packed into blocks here.
    pub fn take_outgoing(&mut self) -> Option<Vec<u8>> {
        if let State::Up(live) = &mut self.state
            && let Err(e) = self.outbox.flush(&mut live.session, &mut self.link)
        {
            self.broken.get_or_insert(e);
        }
        self.link.take_outgoing()
    }

    /// Ends the session from our side: close_notify goes out, and every waiting command is answered `Closed`.
    pub fn close(&mut self) -> Vec<SmpEventKind> {
        let mut events = Vec::new();
        if matches!(self.state, State::Down) {
            return events;
        }
        let _ = self.link.close(); // the pipe is going away; a lost close_notify changes nothing
        self.state = State::Down;
        self.fail_waiting(&SmpError::Closed, &mut events);
        events
    }

    /// Marks the session down: waiting commands get `NotConnected`, then the owner hears why.
    pub(super) fn go_down(&mut self, why: SmpError, events: &mut Vec<SmpEventKind>) {
        let was_up = self.is_up();
        self.state = State::Down;
        self.fail_waiting(&SmpError::NotConnected, events);
        events.push(if was_up {
            SmpEventKind::Down(why)
        } else {
            SmpEventKind::ConnectFailed(why)
        });
    }

    fn fail_waiting(&mut self, err: &SmpError, events: &mut Vec<SmpEventKind>) {
        let pending = self.outbox.drain().into_iter().filter_map(|w| match w {
            Waiter::Tagged(tag) => Some(tag),
            Waiter::Ping => None,
        });
        let tags: Vec<u64> = pending
            .chain(self.queued.drain(..).map(|(_, tag)| tag))
            .collect();
        events.extend(tags.into_iter().map(|tag| SmpEventKind::Reply {
            tag,
            answer: Err(err.clone()),
        }));
    }
}

pub(super) fn info(s: &Established) -> SessionInfo {
    SessionInfo {
        version: s.version,
        session_id: s.session_id.clone(),
        server_info: s.server_info.clone(),
        block_encrypted: s.snd.encrypted(),
    }
}
