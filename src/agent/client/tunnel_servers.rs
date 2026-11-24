//! The relays without threads: the threaded `Servers`' calls, made on a `TunnelPool` the runtime feeds.
use std::sync::Arc;

use crate::smp::{Clock, Request, SharedPool, SmpError, SmpServer};

pub(crate) struct Servers {
    pool: SharedPool,
    clock: Arc<dyn Clock>,
}

impl Servers {
    pub(crate) fn new(pool: SharedPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    pub(crate) fn send(
        &mut self,
        server: &SmpServer,
        request: Request,
        tag: u64,
    ) -> Result<(), SmpError> {
        let now = self.clock.now_ms();
        // The runtime never holds the pool while it calls into the agent, so this is free.
        let mut pool = self.pool.try_borrow_mut().map_err(|_| SmpError::Busy)?;
        pool.send(server, request, tag, now)
    }

    pub(crate) fn retain(&mut self, keep: impl Fn(&SmpServer) -> bool) {
        if let Ok(mut pool) = self.pool.try_borrow_mut() {
            pool.retain(keep);
        }
    }

    pub(crate) fn close_all(&mut self) {
        if let Ok(mut pool) = self.pool.try_borrow_mut() {
            pool.close_all();
        }
    }
}
