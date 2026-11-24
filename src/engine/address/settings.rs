//! The address's settings and showing it in the profile (Commands.hs `APISetAddressSettings`, `APISetProfileAddress`; Subscriber.hs the request's `autoAccept`, `JOINED`'s `sendAutoReply`).

use crate::types::Event;
use crate::types::model::ChatRef;

use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::link::Outgoing;
use crate::engine::machine::Engine;
use crate::engine::records::NS;

/// The auto-reply waiting for a friend made through the address to connect. Eviction: when it
/// goes, or with the friend.
pub(crate) fn reply_key(friend: i64) -> Vec<u8> {
    format!("areply/{friend}").into_bytes()
}

impl<A: AgentPort> Engine<A> {
    pub(crate) fn set_address_settings(
        &mut self,
        auto_accept: bool,
        auto_reply: Option<String>,
    ) -> Result<()> {
        let mut a = (self.state.address.record.clone())
            .ok_or_else(|| fail(Code::RequestFailed, "no address"))?;
        a.auto_accept = auto_accept;
        a.auto_reply = auto_reply
            .map(|r| r.trim().to_owned())
            .filter(|r| !r.is_empty());
        self.save_address(a)?;
        self.emit(Event::Profile(self.profile_wire()));
        Ok(())
    }

    /// The address shows in the profile, or no longer; each friend is sent the new profile.
    pub(crate) fn share_address(&mut self, on: bool) -> Result<()> {
        let link = match (on, &self.state.address.record) {
            (true, Some(a)) => Some(a.full.clone()),
            (true, None) => return Err(fail(Code::RequestFailed, "no address")),
            (false, _) => None,
        };
        let mut user = self.state.user.clone();
        user.contact_link = link;
        self.update_profile(user)
    }

    /// A request answered as it came, and the auto-reply kept for when the friend connects.
    pub(crate) fn auto_answer(&mut self, request: i64) -> Result<()> {
        let Some(a) = self.state.address.record.clone().filter(|a| a.auto_accept) else {
            return Ok(());
        };
        let req = self.request_of(request)?;
        if req.member.is_some() {
            return Ok(());
        }
        let friend = self.accept_address_request(request, req, None)?;
        if let Some(text) = a.auto_reply {
            self.put(&reply_key(friend), &text)?;
        }
        Ok(())
    }

    /// A friend made through the address connected: the auto-reply goes.
    pub(crate) fn send_auto_reply(&mut self, friend: i64) -> Result<()> {
        let Some(bytes) = self.store.get(NS, &reply_key(friend))? else {
            return Ok(());
        };
        self.store.delete(NS, &reply_key(friend))?;
        let text: String = crate::engine::records::decode(&bytes)?;
        self.send_text(
            ChatRef::Direct(friend),
            &text,
            None,
            &[],
            Outgoing::default(),
        )
    }
}
