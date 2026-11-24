//! A job's next move: the next command, a link to its server, the command out, its answer in (Pool.hs `run`, without threads).
use super::super::mem_files::MemFiles;
use super::super::transfers::FileDone;
use super::job::Job;
use crate::smp::{Dialer, PipeId};
use crate::xftp::{Answer, SessionEvent, Step, XftpError};

/// The first wait after a connection failed; it doubles up to `MAX_WAIT`.
const FIRST_WAIT: u64 = 1_000;
const MAX_WAIT: u64 = 30_000;

/// What driving a job needs from the runner.
pub(super) struct Ctx<'a> {
    pub(super) dialer: &'a mut dyn Dialer,
    pub(super) next_pipe: &'a mut PipeId,
    pub(super) mem: &'a MemFiles,
    pub(super) now: u64,
}

impl Job {
    /// Moves the job as far as it goes without the network; its result once it is over.
    pub(super) fn drive(&mut self, cx: &mut Ctx<'_>) -> Option<FileDone> {
        loop {
            if self.is_cancelled() {
                return Some(self.finish(Err(XftpError::Cancelled), cx.mem));
            }
            if let Some(e) = self.failed.take() {
                return Some(self.finish(Err(e), cx.mem));
            }
            if self.call.is_none() {
                match self.work.next_step() {
                    Ok(Step::Call(call)) => self.call = Some((call, false)),
                    Ok(Step::Done(out)) => return Some(self.finish(Ok(out), cx.mem)),
                    Err(e) => return Some(self.finish(Err(e), cx.mem)),
                }
            }
            let Some((call, sent)) = &mut self.call else {
                continue;
            };
            let stale = (self.link.as_ref())
                .is_some_and(|l| l.server != call.server || l.session.is_down());
            if stale && let Some(link) = self.link.take() {
                link.close();
            }
            let Some(link) = &mut self.link else {
                if cx.now < self.retry_at {
                    return None;
                }
                *cx.next_pipe += 1;
                match super::link::FileLink::dial(cx.dialer, *cx.next_pipe, &call.server, cx.now) {
                    Ok(link) => self.link = Some(link),
                    Err(e) => self.answer(Err(e), false, cx.now),
                }
                continue;
            };
            if *sent || !link.session.is_idle() {
                return None;
            }
            let c = &**call;
            match (link.session).send(
                c.key.as_ref(),
                &c.entity,
                &c.cmd,
                &c.file,
                c.max_file,
                cx.now,
            ) {
                Ok(()) => *sent = true,
                Err(e) => self.answer(Err(e), false, cx.now),
            }
        }
    }

    /// What the link's session said: an answer, or that it ended before the command went out,
    /// which counts as a failed try of the command.
    pub(super) fn on_events(&mut self, events: Vec<SessionEvent>, limited: bool, now: u64) {
        for event in events {
            match event {
                SessionEvent::Ready => self.failed_dials = 0,
                SessionEvent::Answer(answer) => self.answer(answer, limited, now),
                SessionEvent::Closed(e) if self.call.as_ref().is_some_and(|(_, sent)| !sent) => {
                    self.answer(Err(e), limited, now);
                }
                SessionEvent::Closed(_) => {}
            }
        }
        let in_flight = (self.link.as_ref())
            .filter(|_| self.call.as_ref().is_some_and(|(_, sent)| *sent))
            .map(|l| l.session.progress());
        if let Some((done, total)) = self.work.progress(in_flight) {
            self.throttle.tell(done, total, &self.tx);
        }
    }

    /// The command in hand is answered. A temporary failure drops the link so the next try is
    /// on a fresh one; a failed connection also waits before the next dial.
    fn answer(&mut self, answer: Answer, limited: bool, now: u64) {
        self.call = None;
        if let Err(e) = &answer {
            if e.is_temporary()
                && let Some(link) = self.link.take()
            {
                link.close();
            }
            if matches!(e, XftpError::Connect(_)) || limited {
                self.failed_dials += 1;
                let wait = FIRST_WAIT << self.failed_dials.min(5).saturating_sub(1);
                self.retry_at = now
                    + if limited {
                        MAX_WAIT
                    } else {
                        wait.min(MAX_WAIT)
                    };
            }
        }
        if let Err(e) = self.work.on_answer(answer) {
            self.failed = Some(e);
        }
    }
}
