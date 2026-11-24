//! One run: socket reports into the pool, SMP events and requests into the engine, timers, then what JS hears.
use std::collections::VecDeque;

use super::clock::JsClock;
use super::files::Parts;
use super::outcome::Outcome;
use super::payload::Payload;
use super::runtime::Core;
use crate::engine::Clock;
use crate::smp::{PipeId, PipeInput};
use crate::types::Request;

impl Core {
    pub(super) fn run(
        &mut self,
        inbox: VecDeque<(PipeId, PipeInput)>,
        requests: VecDeque<(Request, Option<Parts>)>,
    ) -> Outcome {
        let now = self.smp_clock.now_ms();
        if let Ok(mut pool) = self.pool.try_borrow_mut() {
            for (id, input) in inbox {
                if super::files::Files::owns(id) {
                    self.files.on_input(id, input, now);
                } else {
                    pool.on_input(id, input, now);
                }
            }
            pool.tick(now);
        }
        self.settle_net();
        let mut verdicts = Outcome::default();
        for (r, parts) in requests {
            let done = self.engine.handle(r);
            if let Some(parts) = parts {
                verdicts.verdict(parts, &done);
            }
            self.settle_net();
        }
        let wall = JsClock.wall_ms();
        if wall >= self.engine_due {
            self.engine.tick();
            self.settle_net();
        }
        let sleep = i64::try_from(self.engine.sleep_for().as_millis()).unwrap_or(i64::MAX);
        self.engine_due = JsClock.wall_ms().saturating_add(sleep);
        let mut out = self.outcome(wall);
        out.settled.splice(0..0, verdicts.settled);
        out
    }

    /// Bytes out, then the SMP events and job results they and the engine caused, until quiet.
    fn settle_net(&mut self) {
        loop {
            let now = self.smp_clock.now_ms();
            if let Ok(mut pool) = self.pool.try_borrow_mut() {
                pool.flush(now);
                for line in pool.take_notes() {
                    self.log.line(format_args!("tunnel: {line}"));
                }
            }
            self.files.pump(now);
            let mut quiet = true;
            while let Ok(ev) = self.smp.try_recv() {
                self.engine.on_smp(ev);
                quiet = false;
            }
            while let Ok(done) = self.jobs.try_recv() {
                self.engine.on_job(done);
                quiet = false;
            }
            if quiet {
                return;
            }
        }
    }

    /// Every event the engine has for JS, the calls they answer, and calls out of time.
    fn outcome(&mut self, wall: i64) -> Outcome {
        let mut out = Outcome::default();
        while let Some(e) = self.engine.pop_out() {
            out.settled.extend(self.waiters.answer(&e));
            self.hear(&e, &mut out);
            self.files.hear(&e, &mut out, &mut self.engine);
        }
        out.settled.extend(self.waiters.expire(wall));
        let rev = self.engine.store.revision();
        if rev != self.seen_rev {
            self.seen_rev = rev;
            out.calls.push(("change", Payload::Nothing));
        }
        out
    }
}
