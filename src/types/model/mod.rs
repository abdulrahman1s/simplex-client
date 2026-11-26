//! Wire copies of core's `model::chat` types; times are milliseconds since the epoch.

pub mod body;
pub mod bytes;
pub mod call;
pub mod channel;
pub mod chat_features;
pub mod chat_ref;
pub mod conn_event;
pub mod friend;
pub mod group;
pub mod group_prefs;
pub mod link;
pub mod message;
pub mod picture;
pub mod prefs;
mod preview;
pub mod profile;
pub mod send_status;
pub mod stream;

pub use body::{CallKind, CallRecord, ChatNotice, FileState, Media, MessageBody, StreamRecord};
pub use call::{CallKey, CallRequest, CallSignal};
pub use channel::{
    ChannelInfo, ChannelRelay, ChatRelay, RelayStatus, RelayTestFailure, RelayTestResult,
    RelayTestStep,
};
pub use chat_features::ChatFeatures;
pub use chat_ref::ChatRef;
pub use conn_event::{DecryptError, IntegrityError, SwitchPhase, SyncStatus};
pub use friend::{Friend, FriendRequest};
pub use group::{ChatGroup, GroupMember, MemberRole, Membership};
pub use group_prefs::{GroupPref, GroupPrefs};
pub use link::LinkInfo;
pub use message::{ChatMessage, Delivery, Mention, MessageAuthor, Quote, Reaction, Timed};
pub use picture::Picture;
pub use prefs::{Allow, ChatFeature, ChatPrefs, PrefEnabled};
pub use preview::ChatPreview;
pub use profile::{ChatProfile, MyStatus, Playing, Presence};
pub use send_status::SendStatus;
pub use stream::{StreamRequest, StreamSignal};
