//! The agent version a connection runs at: our envelopes offer our highest, the peer's raise the stored one (Agent.hs `updateConnVersion`, `runSmpQueueMsgDelivery`).
use crate::encoding::{Version, VersionRange};
use crate::store::Store;

use crate::agent::client::{AgentClient, ConnId};
use crate::agent::error::Result;
use crate::agent::records::Batch;

impl AgentClient {
    /// `maxVersion smpAgentVRange`, whatever the connection agreed: it is how the peer learns it can upgrade.
    pub(crate) fn envelope_agent_version(&self) -> Version {
        self.cfg.agent_versions.max()
    }

    /// `updateConnVersion`, saved on its own so that a message that then fails to decrypt still counts.
    pub(crate) fn update_conn_version(
        &mut self,
        store: &mut Store,
        id: ConnId,
        msg_v: Version,
    ) -> Result<()> {
        let agreed = self.conn(id)?.rec.agent_version;
        let Some(v) = upgraded_version(&self.cfg.agent_versions, agreed, msg_v) else {
            return Ok(());
        };
        let (mut rec, _) = self.editable(id)?;
        rec.agent_version = v;
        self.save(store, rec, None, Batch::default())
    }
}

/// The highest version both sides speak, when it is above the agreed one; a connection never goes down.
fn upgraded_version(ours: &VersionRange, agreed: Version, msg_v: Version) -> Option<Version> {
    // `safeVersionRange`: a version below our minimum stands alone, and so matches nothing.
    let theirs =
        VersionRange::new(ours.min(), msg_v).unwrap_or_else(|| VersionRange::single(msg_v));
    theirs.highest_common(ours).filter(|&v| v > agreed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(min: u16, max: u16) -> VersionRange {
        VersionRange::new(Version(min), Version(max)).unwrap()
    }

    #[test]
    fn a_higher_peer_version_raises_the_agreed_one_up_to_ours() {
        assert_eq!(
            upgraded_version(&range(6, 8), Version(7), Version(8)),
            Some(Version(8))
        );
        assert_eq!(
            upgraded_version(&range(6, 8), Version(6), Version(9)),
            Some(Version(8))
        );
    }

    #[test]
    fn the_agreed_version_never_goes_down_nor_past_what_we_speak() {
        assert_eq!(upgraded_version(&range(6, 8), Version(8), Version(7)), None);
        assert_eq!(upgraded_version(&range(6, 8), Version(7), Version(7)), None);
        assert_eq!(upgraded_version(&range(6, 7), Version(7), Version(8)), None);
        assert_eq!(upgraded_version(&range(6, 8), Version(7), Version(5)), None);
    }
}
