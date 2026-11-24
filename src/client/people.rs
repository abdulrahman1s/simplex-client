//! Friends and the user: links, requests, security codes and the profile.

use std::sync::PoisonError;

use super::error::{Result, unexpected};
use super::handle::Client;
use super::waiters::{Answer, Want};
use crate::types::model::ChatProfile;
use crate::types::{Event, Request, RequestKind};

impl Client {
    /// A one-time invitation link for one person.
    pub fn create_invite(&self) -> Result<String> {
        let Answer::Event(Event::Invite(link)) = self.ask(Want::Invite, Request::CreateInvite)?
        else {
            return Err(unexpected(RequestKind::CreateInvite));
        };
        Ok(link)
    }

    /// The user's long-lived address, made on first use; anyone may ask to connect through it.
    pub fn create_address(&self) -> Result<String> {
        let Answer::Event(Event::Address(Some(link))) =
            self.ask(Want::Address, Request::CreateAddress)?
        else {
            return Err(unexpected(RequestKind::Address));
        };
        Ok(link)
    }

    /// The address stops working; requests waiting on it are forgotten.
    pub fn delete_address(&self) -> Result<()> {
        self.request(Request::DeleteAddress)
    }

    /// Connects through an invitation, an address or a group link.
    pub fn connect(&self, link: &str) -> Result<()> {
        self.request(Request::Connect(link.to_owned()))
    }

    /// Accepts a request by its id; `Event::Request` carries one that can `accept` itself.
    pub fn accept(&self, request: i64) -> Result<()> {
        self.request(Request::Accept(request))
    }

    pub fn reject(&self, request: i64) -> Result<()> {
        self.request(Request::Reject(request))
    }

    pub fn remove_friend(&self, friend: i64) -> Result<()> {
        self.request(Request::RemoveFriend(friend))
    }

    /// The security code to compare with the friend's, out of band.
    pub fn show_code(&self, friend: i64) -> Result<String> {
        let Answer::Event(Event::Code { code, .. }) =
            self.ask(Want::Code(friend), Request::ShowCode(friend))?
        else {
            return Err(unexpected(RequestKind::Code));
        };
        Ok(code)
    }

    pub fn set_name(&self, name: &str) -> Result<()> {
        let mut profile = self.profile();
        name.clone_into(&mut profile.name);
        self.set_profile(profile)
    }

    /// Replaces the profile; friends get the new one.
    pub fn set_profile(&self, profile: ChatProfile) -> Result<()> {
        // Cached now, so a `set_name` right after builds on this profile, not the old one.
        self.remember_profile(&profile);
        self.request(Request::SetProfile(profile))
    }

    fn remember_profile(&self, profile: &ChatProfile) {
        let p = self.inner.shared.profile.lock();
        profile.clone_into(&mut p.unwrap_or_else(PoisonError::into_inner));
    }
}
