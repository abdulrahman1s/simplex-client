//! The client's vocabulary: what you ask ([`Request`]), what you hear ([`Event`]) and the models both carry.

pub mod config;
pub mod event;
pub mod features;
pub mod kind;
pub mod model;
pub mod request;

pub use config::Config;
pub use event::Event;
pub use features::{REACTION_EMOJI, features_with, has_file_servers};
pub use kind::{ErrorCode, FileKind, RequestKind};
pub use request::Request;
