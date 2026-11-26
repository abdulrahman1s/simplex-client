//! The roster handshake with a relay (chat v19): the owner sends its signed roster header and the blob as `F` chunks; the relay's ack makes it publishable (Internal.hs `sendGroupRosterToRelay`, `sendRoster`, `sendInlineBlobChunks`; Protocol.hs `encodeRosterBlob`; Subscriber.hs `xGrpRosterAck`).

use crate::chat::channel::InlineFileInvitation;
use crate::chat::element::{encode_file_chunk_element, encode_signed_element};
use crate::chat::event::FileChunk;
use crate::chat::ids::b64_encode;
use crate::chat::{ChatEvent, ChatMessage};
use crate::crypto::sha512;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{MemberStatus, RelayStatus};

use super::keys::{binding, sign};

/// `groupRosterVersion`: a relay below it can't ack a roster and is published on connect.
pub(super) const ROSTER_VERSION: u16 = 19;

/// `encodeRosterBlob []`: the members besides owners with roster roles. This engine's
/// channels have none, so the blob is the count 0 as a `Word16`.
const EMPTY_ROSTER: [u8; 2] = [0, 0];

impl<A: AgentPort> Engine<A> {
    /// `CON` with a relay: the roster first, or straight to publishing for an older relay.
    pub(super) fn on_owner_relay_con(&mut self, group: i64, member: i64) -> Result<()> {
        let mut m = self.member(group, member)?;
        m.status = MemberStatus::Connected;
        let rosters = m.chat_v.is_none_or(|v| v.max >= ROSTER_VERSION);
        self.save_member(m)?;
        if rosters {
            return self.send_roster(group, member);
        }
        self.publish_channel(group)
    }

    /// `sendRoster`: `x.grp.roster` signed (it is in `requiresSignature`), then the blob under its id.
    fn send_roster(&mut self, group: i64, member: i64) -> Result<()> {
        let g = self.group(group)?;
        let c = g
            .channel
            .clone()
            .ok_or_else(|| fail(Code::RequestFailed, "not a channel"))?;
        let conn = self
            .member(group, member)?
            .reachable()
            .ok_or_else(|| fail(Code::RequestFailed, "relay not connected"))?;
        let msg = ChatMessage::new(ChatEvent::GrpRoster {
            version: c.roster_version.unwrap_or(0),
            file_inv: InlineFileInvitation {
                file_size: EMPTY_ROSTER.len() as u64,
                file_digest: b64_encode(&sha512(&EMPTY_ROSTER)),
            },
        })?;
        let shared = msg
            .msg_id
            .clone()
            .ok_or_else(|| fail(Code::RequestFailed, "no message id"))?;
        let signed = sign(
            &c.member_private,
            &binding(&c.public_group_id, &g.my_member_id)?,
            msg.to_json()?,
        )?;
        let header = encode_signed_element(&signed)?;
        let chunk = FileChunk::Chunk {
            chunk_no: 1,
            bytes: EMPTY_ROSTER.to_vec(),
        };
        let blob = encode_file_chunk_element(&shared, &chunk)?;
        for body in [header, blob] {
            self.agent_send(&conn, false, body)?;
        }
        Ok(())
    }

    /// `xGrpRosterAck`: an accepted relay that took the roster is published; one that refused it is rejected.
    pub(super) fn on_roster_ack(
        &mut self,
        group: i64,
        member: i64,
        error: Option<String>,
    ) -> Result<()> {
        let g = self.group(group)?;
        let accepted = g
            .channel
            .as_ref()
            .and_then(|c| c.relay(member))
            .is_some_and(|r| r.status == RelayStatus::Accepted);
        if !accepted {
            return Ok(());
        }
        if let Some(e) = error {
            self.log
                .line(format_args!("a chat relay refused the roster: {e}"));
            return self.set_relay_status(group, member, RelayStatus::Rejected);
        }
        self.set_relay_status(group, member, RelayStatus::AcknowledgedRoster)?;
        self.publish_channel(group)
    }
}
