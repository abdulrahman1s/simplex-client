//! A connection's ratchet-sync state changed (`RSYNC`): told in the chat, a required one started, and a friend's new ratchet has a new security code (Subscriber.hs `RSYNC`, `ratchetSyncEventItem`, `securityCodeChanged`; Commands.hs `APISyncContactRatchet`).

use crate::agent::resync::SyncState;
use crate::types::model::{ChatNotice, SyncStatus};

use super::conn_event::By;
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// The agent reports `Allowed` and `Required` only with a decryption error, which chat shows
    /// as a decryption-error item, not a sync one; members are never verified here.
    pub(crate) fn on_ratchet_sync(&mut self, conn: &ConnId, state: SyncState) -> Result<()> {
        self.log.line(format_args!(
            "connection {}: ratchet sync {state:?}",
            conn.0
        ));
        let status = match state {
            SyncState::Ok => SyncStatus::Ok,
            SyncState::Agreed => SyncStatus::Agreed,
            SyncState::Required => return self.repair(conn),
            SyncState::Allowed | SyncState::Started => return Ok(()),
        };
        self.conn_event(conn, By::Peer, |member| ChatNotice::RatchetSync {
            status,
            member,
        })?;
        if let (SyncState::Agreed, Some(friend)) = (state, self.state.friend_of(conn)) {
            return self.security_code_changed(friend);
        }
        Ok(())
    }

    /// The apps offer "fix connection" here and refuse to send until it is pressed; with no such
    /// button the re-sync starts at once, told as the user's like `APISyncContactRatchet`. The
    /// agent reports `Required` once per failure, and `Started` waits for the peer, so this
    /// cannot loop.
    fn repair(&mut self, conn: &ConnId) -> Result<()> {
        if let Err(e) = self.agent.synchronize_ratchet(&mut self.store, conn, false) {
            self.log.line(format_args!(
                "connection {}: could not start a re-sync: {}",
                conn.0,
                e.detail()
            ));
            return Ok(());
        }
        self.conn_event(conn, By::User, |member| ChatNotice::RatchetSync {
            status: SyncStatus::Started,
            member,
        })
    }

    /// SyncRatchet: the user asks for a new ratchet with a friend or a member, as the apps'
    /// "fix connection" (`APISyncContactRatchet`, `APISyncGroupMemberRatchet`); `force` asks
    /// even while the connection works.
    pub(crate) fn sync_ratchet(&mut self, conn: &ConnId, force: bool) -> Result<()> {
        self.agent
            .synchronize_ratchet(&mut self.store, conn, force)
            .map_err(|e| {
                crate::engine::error::fail(crate::engine::error::Code::RequestFailed, e.detail())
            })?;
        self.conn_event(conn, By::User, |member| ChatNotice::RatchetSync {
            status: SyncStatus::Started,
            member,
        })
    }
}
