//! Messages for a member whose connection is not up yet: kept, and sent once it is (Internal.hs `sendPendingGroupMessages`; Store/Messages.hs `createPendingGroupMessage`, `getPendingGroupMessages`).

use crate::types::model::ChatRef;

use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::pending::{MAX_PENDING, PENDING_TTL_MS};
use crate::engine::records::{NS, PendingMsg, decode, keys};

impl<A: AgentPort> Engine<A> {
    fn pending_of(&mut self, group: i64, member: i64) -> Result<Vec<PendingMsg>> {
        let bytes = self.store.get(NS, &keys::pending(group, member))?;
        Ok(bytes.map(|b| decode(&b)).transpose()?.unwrap_or_default())
    }

    pub(crate) fn queue_pending(
        &mut self,
        group: i64,
        member: i64,
        body: &[u8],
        shared: Option<&[u8]>,
    ) -> Result<()> {
        let now = self.clock.wall_ms();
        let mut queue = self.pending_of(group, member)?;
        queue.retain(|p| now - p.at < PENDING_TTL_MS);
        queue.push(PendingMsg {
            body: body.to_vec(),
            at: now,
            shared: shared.map(<[u8]>::to_vec),
        });
        let over = queue.len().saturating_sub(MAX_PENDING);
        queue.drain(..over);
        self.put(&keys::pending(group, member), &queue)?;
        Ok(())
    }

    /// On the member's CON, before anything else: what waited goes out on `conn`, oldest
    /// first, and the user's items it carried are indexed for their delivery. The items it
    /// carried, sent or not.
    pub(crate) fn send_pending(
        &mut self,
        group: i64,
        member: i64,
        conn: ConnId,
    ) -> Result<Vec<i64>> {
        let queue = self.pending_of(group, member)?;
        if queue.is_empty() {
            return Ok(Vec::new());
        }
        self.store.delete(NS, &keys::pending(group, member))?;
        let now = self.clock.wall_ms();
        let mut items = Vec::new();
        for p in queue {
            let item = match &p.shared {
                Some(s) => self.item_by_shared(ChatRef::Group(group), None, s)?,
                None => None,
            };
            items.extend(item.as_ref().map(|(id, _)| *id));
            if now - p.at >= PENDING_TTL_MS {
                continue;
            }
            match self
                .agent
                .send(&mut self.store, &conn, self.state.pq_of(&conn), p.body)
            {
                Ok(sent) => {
                    if let Some((id, _)) = item {
                        self.put_index(&keys::sent(&conn, sent), id)?;
                    }
                }
                Err(e) => self.log.line(format_args!(
                    "a pending message was not sent: {}",
                    e.detail()
                )),
            }
        }
        Ok(items)
    }

    /// With the member's connection: nothing can carry them any more.
    pub(crate) fn drop_pending(&mut self, group: i64, member: i64) -> Result<()> {
        self.store.delete(NS, &keys::pending(group, member))?;
        Ok(())
    }
}
