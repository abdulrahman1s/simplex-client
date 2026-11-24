//! The engine's records in the store, namespace "chat", versioned JSON.

pub mod address;
pub mod conn;
pub mod file;
pub mod format;
pub mod friend;
pub mod group;
pub mod item;
pub mod keys;
pub mod local;
pub mod member;
pub mod pending;
pub mod settings;
pub mod user;

pub use address::{AddressRecord, MemberInvite, RequestRecord};
pub use conn::{ConnKind, ConnRecord};
pub use format::{NS, VERSION, decode, encode};
pub use friend::FriendRecord;
pub use group::{GroupRecord, MemberStatus};
pub use item::ItemRecord;
pub use local::ChatLocal;
pub use member::{MemberCategory, MemberRecord, MemberRelation};
pub use pending::PendingMsg;
pub use settings::SettingsRecord;
pub use user::UserRecord;
