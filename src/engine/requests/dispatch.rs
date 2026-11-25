//! The request table: the limits first, then each request to its handler; a handler's error answers `Failed`.

use crate::types::{Event, Request};

use super::kind::kind_of;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, EngineError, Result, fail};
use crate::engine::files::FileMeta;
use crate::engine::link::Outgoing;
use crate::engine::machine::Engine;
use crate::limits::check;

impl<A: AgentPort> Engine<A> {
    pub fn on_request(&mut self, request: Request) {
        // The failure has gone out as `Event::Failed`; nobody else waits for it here.
        let _ = self.handle(request);
    }

    /// `on_request` that also hands back the failure it announced.
    pub fn handle(&mut self, request: Request) -> Result<()> {
        if let Request::Tagged { tag, request } = request {
            return self.handle_tagged(tag, *request);
        }
        let kind = kind_of(&request);
        let done = check(&request)
            .map_err(EngineError::from)
            .and_then(|()| self.route(request));
        if let Err(e) = &done {
            self.emit(Event::Failed {
                request: kind,
                code: e.code,
                detail: e.detail.clone(),
            });
        }
        self.pump();
        done
    }

    pub(super) fn route(&mut self, request: Request) -> Result<()> {
        match request {
            Request::Tagged { .. } => Err(fail(Code::RequestFailed, "a tagged request in another")),
            Request::SetProfile(p) => self.set_profile(&p),
            Request::SetPicture(p) => self.set_picture(p),
            Request::Accept(id) => self.accept_request(id, false),
            Request::AcceptIncognito(id) => self.accept_request(id, true),
            Request::Reject(id) => self.reject_request(id),
            Request::CreateAddress => self.create_address(),
            Request::DeleteAddress => self.delete_address(),
            Request::SendFile {
                chat,
                path,
                kind,
                preview,
                duration,
                text,
            } => {
                let meta = FileMeta {
                    preview: preview.as_deref(),
                    duration,
                    text: &text,
                    forward: false,
                };
                self.send_file(chat, &path, kind, meta)
            }
            Request::ReceiveFile { chat, item } => self.receive_file(chat, item),
            Request::CancelFile { chat, item } => self.cancel_file(chat, item),
            Request::OpenFile { chat, item } => self.open_file(chat, item),
            Request::LocateFile { chat, item, tag } => self.locate_file(chat, item, tag),
            Request::ShowCode(friend) => self.show_code(friend),
            Request::Verify { friend, code } => self.verify(friend, &code),
            Request::CreateInvite => self.create_invite(false),
            Request::CreateIncognitoInvite => self.create_invite(true),
            Request::Connect(link) => self.connect(&link, false),
            Request::ConnectIncognito(link) => self.connect(&link, true),
            Request::RemoveFriend(id) => self.remove_friend(id),
            Request::Send {
                chat,
                text,
                reply_to,
                mentions,
            } => self.send_text(chat, &text, reply_to, &mentions, Outgoing::default()),
            Request::SendData {
                chat,
                tag,
                json,
                fallback_text,
            } => self.send_data(chat, &tag, &json, &fallback_text),
            Request::SendLink {
                chat,
                text,
                preview,
                reply_to,
            } => {
                let out = Outgoing {
                    link: Some(&preview),
                    ..Outgoing::default()
                };
                self.send_text(chat, &text, reply_to, &[], out)
            }
            Request::MarkRead(chat) => self.mark_read(chat),
            Request::History {
                chat,
                before,
                count,
                tag,
            } => self.history(chat, before, count, tag),
            Request::Refresh => {
                self.state.chats_dirty = true;
                Ok(())
            }
            Request::SetMuted { chat, on } => self.set_muted(chat, on),
            Request::SetTimer { chat, ttl } => self.set_timer(chat, ttl),
            Request::SetPrefs { friend, prefs } => self.set_prefs(friend, &prefs),
            Request::SetMemberRole {
                group,
                member,
                role,
            } => self.set_member_role(group, member, role),
            Request::BlockMemberForAll { group, member, on } => {
                self.block_member_for_all(group, member, on)
            }
            Request::RemoveMember {
                group,
                member,
                messages,
            } => self.remove_member(group, member, messages),
            Request::DeleteGroup(group) => self.delete_group(group),
            Request::SetGroupProfile {
                group,
                name,
                description,
            } => self.set_group_profile(group, &name, description.as_deref()),
            Request::SetGroupPrefs { group, prefs } => self.set_group_prefs(group, &prefs),
            Request::Moderate { group, item } => self.moderate(group, item),
            Request::DeleteForMe { chat, item } => self.delete_for_me(chat, item),
            Request::Forward { from, item, to } => self.forward(from, item, to),
            Request::SyncRatchet {
                chat,
                member,
                force,
            } => {
                let conn = self.conn_of(chat, member)?;
                self.sync_ratchet(&conn, force)
            }
            Request::SetAddressSettings {
                auto_accept,
                auto_reply,
            } => self.set_address_settings(auto_accept, auto_reply),
            Request::ShareAddress(on) => self.share_address(on),
            Request::ContactMember {
                group,
                member,
                text,
            } => self.contact_member(group, member, text),
            Request::SendLive { chat, text } => {
                let live = Outgoing {
                    live: true,
                    ..Outgoing::default()
                };
                self.send_text(chat, &text, None, &[], live)
            }
            Request::UpdateLive {
                chat,
                item,
                text,
                done,
            } => self.update_live(chat, item, &text, done),
            Request::ClearChat(chat) => self.clear_chat(chat),
            Request::MarkReadUpTo { chat, item } => self.read_to(chat, Some(item)),
            Request::SetUnreadMark { chat, on } => self.set_unread_mark(chat, on),
            Request::SetAlias { chat, alias } => self.set_alias(chat, &alias),
            Request::SetMessageTtl { chat, ttl } => self.set_message_ttl(chat, ttl),
            Request::SetBlocked { friend, on } => self.set_blocked(friend, on),
            Request::SetMemberBlocked { group, member, on } => {
                self.set_member_blocked(group, member, on)
            }
            Request::Edit { chat, item, text } => self.edit(chat, item, &text),
            Request::Delete { chat, item } => self.delete(chat, item),
            Request::React {
                chat,
                item,
                emoji,
                on,
            } => self.react(chat, item, &emoji, on),
            Request::Call(r) => self.call(r),
            Request::Stream(r) => self.stream(r),
            Request::SetGroupPicture { group, picture } => self.set_group_picture(group, picture),
            Request::CreateGroupLink(group) => self.create_group_link(group),
            Request::DeleteGroupLink(group) => self.delete_group_link(group),
            Request::Members(group) => self.members(group),
            Request::NewGroup(name) => self.new_group(&name),
            Request::AddMember { group, friend } => self.add_member(group, friend),
            Request::JoinGroup(group) => self.join_group(group),
            Request::LeaveGroup(group) => self.leave_group(group),
        }
    }
}
