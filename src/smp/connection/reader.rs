//! The reader thread of one session: matches answers to waiting commands by corrId and forwards pushes (Client.hs `receiveSMP`).
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::smp::block::RcvBlocks;
use crate::smp::error::SmpError;
use crate::smp::responses::Response;
use crate::smp::transmission::{Incoming, parse_payload};
use crate::smp::transport::Reader;

use super::shared::{Shared, lock};
use crate::smp::flow::route;

/// Reads blocks until the connection fails; returns why.
pub fn run(
    sh: &Arc<Shared>,
    mut reader: Box<dyn Reader>,
    mut rcv: RcvBlocks,
    version: u16,
) -> SmpError {
    loop {
        let payload = match rcv.read(&mut *reader) {
            Ok(p) => p,
            Err(e) => return e,
        };
        sh.last_rx.store(sh.clock.now_ms(), Ordering::Release);
        let transmissions = match parse_payload(version, &payload) {
            Ok(t) => t,
            Err(e) => return e.into(),
        };
        for t in transmissions {
            if !dispatch(sh, version, t) {
                return SmpError::Closed;
            }
        }
    }
}

/// False when the owner stopped listening.
fn dispatch(sh: &Shared, version: u16, t: Incoming) -> bool {
    let decoded = Response::decode(version, &t.command);
    let waiting = if t.corr_id.is_empty() {
        None
    } else {
        lock(&sh.pending).take(&t.corr_id)
    };
    let routed = route(
        decoded,
        t.entity_id,
        waiting.as_ref().map(|e| e.entity_id.as_slice()),
    );
    let kept = routed.event.is_none_or(|e| sh.emit(e));
    if let (Some(entry), Some(answer)) = (waiting, routed.answer) {
        sh.answer(&entry.reply, answer);
    }
    kept
}
