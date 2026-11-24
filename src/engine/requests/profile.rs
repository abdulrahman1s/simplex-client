//! SetProfile: the name and the line about the user, kept and sent as `x.info` to every friend.

use crate::types::Event;
use crate::types::model::ChatProfile;

use crate::chat::{ChatEvent, ChatMessage, encode_chat_message};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::profile::{MAX_PICTURE_URI, sent_profile, sent_profile_to};
use crate::engine::records::UserRecord;
use crate::limits::{MAX_BIO_BYTES, is_valid_name, json_len};

impl<A: AgentPort> Engine<A> {
    /// The picture is not part of it (`SetPicture`), so the one set stays.
    /// A changed name, trimmed, must be one upstream keeps as it is (Commands.hs
    /// `updateProfile_`: `checkValidName` when the name changes); a changed bio fits 160 bytes.
    pub(crate) fn set_profile(&mut self, p: &ChatProfile) -> Result<()> {
        let name = p.name.trim();
        if name != self.state.user.name && !is_valid_name(name) {
            return Err(fail(
                Code::InvalidName,
                format!("not a valid name: {name:?}"),
            ));
        }
        let about = p.about.trim();
        // The apps' bio field (`MAX_BIO_LENGTH_BYTES`); a longer one already kept may stay.
        if about != self.state.user.about && json_len(about) > MAX_BIO_BYTES {
            return Err(fail(
                Code::TextTooLong,
                "the line about the user is too long",
            ));
        }
        let user = UserRecord {
            name: name.to_owned(),
            about: about.to_owned(),
            paper: p.paper,
            status: p.status,
            picture: self.state.user.picture.clone(),
            prefs: self.state.user.prefs.clone(),
        };
        self.update_profile(user)
    }

    /// Commands.hs `updateProfile_`: checked and encoded before anything is kept, nothing sent
    /// when the profile as friends see it did not change.
    pub(crate) fn update_profile(&mut self, user: UserRecord) -> Result<()> {
        let new = sent_profile(&user);
        if let Some(image) = &new.image
            && image.0.len() > MAX_PICTURE_URI
        {
            return Err(fail(
                Code::PictureTooLarge,
                format!("Profile image is too large {}", image.0.len()),
            ));
        }
        let changed = new != sent_profile(&self.state.user);
        if changed {
            // Each friend's copy differs only in their overrides, so one that fits says all fit.
            let msg = ChatMessage::new(ChatEvent::Info {
                profile: new,
                member_key: None,
            })?;
            encode_chat_message(&msg, true)?;
        }
        // Each friend whose profile this changes is sent theirs (`updateProfile_`).
        let told: Vec<i64> = (self.state.friends.values())
            .filter(|f| f.reachable())
            .filter(|f| sent_profile_to(&user, f) != sent_profile_to(&self.state.user, f))
            .map(|f| f.id)
            .collect();
        self.save_user(user)?;
        self.emit(Event::Profile(self.state.user.to_wire()));
        for friend in told {
            // One friend's relay failing is no reason to keep the profile from the rest.
            if let Err(e) = self.send_profile(friend) {
                self.log
                    .line(format_args!("profile not sent: {}", e.detail));
            }
        }
        Ok(())
    }

    /// `x.info` with the profile as this friend sees it, their timer included
    /// (Types.hs `userProfileDirect`).
    pub(crate) fn send_profile(&mut self, friend: i64) -> Result<()> {
        let f = self.friend(friend)?;
        let msg = ChatMessage::new(ChatEvent::Info {
            profile: sent_profile_to(&self.state.user, &f),
            member_key: None,
        })?;
        let body = encode_chat_message(&msg, true)?;
        self.agent.send(&mut self.store, &f.conn, f.pq, body)?;
        Ok(())
    }
}
