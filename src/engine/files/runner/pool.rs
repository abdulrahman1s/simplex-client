//! Transfers without threads: driven by pipe reports and one timer, at most two at a time, each with its own connection.
use std::collections::VecDeque;

use crossbeam_channel::{Receiver, Sender};

use super::super::mem_files::MemFiles;
use super::super::queued::FileJob;
use super::drive::Ctx;
use super::job::Job;
use crate::engine::jobs::JobDone;
use crate::smp::{Dialer, PipeId, PipeInput};

/// Transfers running at once, like the native worker threads; each holds one connection.
const WORKERS: usize = 2;
/// Pipe ids above this are the runner's; the relay pool counts up from 1 and never gets here.
pub(super) const FILE_PIPES: PipeId = 1 << 48;

pub struct FileRunner {
    jobs: Receiver<(FileJob, Sender<JobDone>)>,
    /// Waiting their turn; bounded by the engine, which starts at most `files::MAX_ACTIVE`.
    waiting: VecDeque<(FileJob, Sender<JobDone>)>,
    /// At most `WORKERS`; a job leaves when it posts its result.
    running: Vec<Job>,
    dialer: Box<dyn Dialer>,
    next_pipe: PipeId,
    token: Option<String>,
    mem: MemFiles,
}

impl FileRunner {
    #[must_use]
    pub fn new(
        jobs: Receiver<(FileJob, Sender<JobDone>)>,
        dialer: Box<dyn Dialer>,
        token: Option<String>,
        mem: MemFiles,
    ) -> Self {
        Self {
            jobs,
            waiting: VecDeque::new(),
            running: Vec::new(),
            dialer,
            next_pipe: FILE_PIPES,
            token,
            mem,
        }
    }

    /// Whether pipe `id` is one of the runner's.
    #[must_use]
    pub fn owns(id: PipeId) -> bool {
        id > FILE_PIPES
    }

    /// What pipe `id` reported; news of a pipe no job holds any more is ignored.
    pub fn on_input(&mut self, id: PipeId, input: PipeInput, now: u64) {
        let token = self.token.as_deref();
        let held =
            (self.running.iter_mut()).find(|j| j.link.as_ref().is_some_and(|l| l.carrier.id == id));
        if let Some(job) = held
            && let Some(link) = &mut job.link
        {
            let (events, limited) = link.on_input(input, now, token);
            job.on_events(events, limited, now);
        }
        self.settle(now);
    }

    /// New transfers start, deadlines pass, due connections are dialled.
    pub fn tick(&mut self, now: u64) {
        while let Ok(job) = self.jobs.try_recv() {
            self.waiting.push_back(job);
        }
        for job in &mut self.running {
            if let Some(link) = &mut job.link {
                let events = link.session.tick(now);
                job.on_events(events, false, now);
            }
        }
        self.settle(now);
    }

    /// Sends what the sessions have for their pipes; call after every input.
    pub fn flush(&mut self, now: u64) {
        for job in &mut self.running {
            if let Some(link) = &mut job.link {
                let events = link.flush();
                job.on_events(events, false, now);
            }
        }
        self.settle(now);
    }

    /// When `tick` next has work, if ever.
    #[must_use]
    pub fn next_deadline(&self) -> Option<u64> {
        let each = self.running.iter().map(|j| match &j.link {
            Some(l) => l.session.next_deadline(),
            None => Some(j.retry_at),
        });
        let soonest = each.flatten().min();
        if self.waiting.is_empty() {
            soonest
        } else {
            Some(0)
        }
    }

    /// Every transfer stops; their results go nowhere, as the engine is closing too.
    pub fn close(&mut self) {
        for mut job in self.running.drain(..) {
            if let Some(link) = job.link.take() {
                link.close();
            }
        }
        self.waiting.clear();
    }

    /// Starts waiting transfers in free places and drives every running one; the finished
    /// post their results and make room.
    fn settle(&mut self, now: u64) {
        loop {
            while self.running.len() < WORKERS
                && let Some((job, tx)) = self.waiting.pop_front()
            {
                match Job::start(job, tx.clone(), &self.mem) {
                    Ok(job) => self.running.push(job),
                    Err(done) => drop(tx.send(JobDone::File(done))), // the engine is gone
                }
            }
            let mut cx = Ctx {
                dialer: &mut *self.dialer,
                next_pipe: &mut self.next_pipe,
                mem: &self.mem,
                now,
            };
            let before = self.running.len();
            self.running.retain_mut(|job| match job.drive(&mut cx) {
                Some(done) => {
                    let _ = job.tx.send(JobDone::File(done)); // the engine is gone
                    false
                }
                None => true,
            });
            if self.running.len() == before || self.waiting.is_empty() {
                return;
            }
        }
    }
}
