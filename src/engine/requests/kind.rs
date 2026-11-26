//! Which request a failure answers (the kinds core's codec gives each request).

use crate::types::{Request, RequestKind};

#[must_use]
pub fn kind_of(r: &Request) -> RequestKind {
    use RequestKind as K;
    match r {
        Request::Tagged { request, .. } => kind_of(request),
        Request::SetProfile(_) | Request::SetPicture(_) => K::SetProfile,
        Request::CreateInvite | Request::CreateIncognitoInvite => K::CreateInvite,
        Request::Connect(_) | Request::ConnectIncognito(_) => K::Connect,
        Request::Accept(_) | Request::AcceptIncognito(_) | Request::Reject(_) => K::Answer,
        Request::CreateAddress | Request::DeleteAddress => K::Address,
        Request::Send { .. } => K::Send,
        Request::SendFile { .. } => K::SendFile,
        Request::ReceiveFile { .. }
        | Request::CancelFile { .. }
        | Request::OpenFile { .. }
        | Request::LocateFile { .. } => K::File,
        Request::CreateGroupLink(_) | Request::DeleteGroupLink(_) => K::GroupLink,
        Request::Edit { .. } => K::Edit,
        Request::Delete { .. } => K::Delete,
        Request::React { .. } => K::React,
        Request::SetMuted { .. } => K::Mute,
        Request::SetTimer { .. } => K::Timer,
        Request::SendLink { .. }
        | Request::Forward { .. }
        | Request::SendLive { .. }
        | Request::SendData { .. }
        | Request::SendSignedData { .. } => K::Send,
        Request::ContactMember { .. } | Request::SyncRatchet { .. } => K::Connect,
        Request::SetAddressSettings { .. } | Request::ShareAddress(_) => K::Address,
        Request::UpdateLive { .. } => K::Edit,
        Request::SetPrefs { .. } => K::Prefs,
        Request::SetMemberRole { .. }
        | Request::BlockMemberForAll { .. }
        | Request::RemoveMember { .. }
        | Request::DeleteGroup(_)
        | Request::SetGroupProfile { .. }
        | Request::SetGroupPrefs { .. }
        | Request::Moderate { .. } => K::GroupAdmin,
        Request::DeleteForMe { .. }
        | Request::ClearChat(_)
        | Request::MarkReadUpTo { .. }
        | Request::SetUnreadMark { .. }
        | Request::SetAlias { .. }
        | Request::SetMessageTtl { .. } => K::Local,
        Request::SetBlocked { .. } | Request::SetMemberBlocked { .. } => K::Block,
        Request::ShowCode(_) | Request::Verify { .. } => K::Code,
        Request::Members(_) => K::Members,
        Request::MarkRead(_) => K::MarkRead,
        Request::History { .. } => K::History,
        Request::Refresh => K::Refresh,
        Request::TestChatRelay(_) => K::RelayTest,
        Request::SetGroupPicture { .. }
        | Request::NewGroup(_)
        | Request::NewChannel(_)
        | Request::SetChatRelays(_)
        | Request::AddChannelRelays { .. }
        | Request::AddMember { .. }
        | Request::JoinGroup(_)
        | Request::LeaveGroup(_) => K::Group,
        Request::RemoveFriend(_) => K::RemoveFriend,
        Request::Call(_) | Request::Stream(_) => K::Call,
    }
}
