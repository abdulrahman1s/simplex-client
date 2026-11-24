//! What a request handler fails with: one of the stable `SX-CHT-*` codes and the engine's words.

use super::agent_port::AgentFailure;
use crate::chat::ChatError;
use crate::limits::Refusal;
use crate::store::StoreError;

pub use crate::types::ErrorCode as Code;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineError {
    pub code: Code,
    /// For the log; never message text.
    pub detail: String,
}

pub type Result<T, E = EngineError> = std::result::Result<T, E>;

#[must_use]
pub fn fail(code: Code, detail: impl Into<String>) -> EngineError {
    EngineError {
        code,
        detail: detail.into(),
    }
}

/// A request this engine does not carry yet.
#[must_use]
pub fn not_yet(code: Code) -> EngineError {
    fail(code, "not yet")
}

impl From<Refusal> for EngineError {
    fn from(r: Refusal) -> Self {
        fail(r.code, r.detail)
    }
}

impl From<StoreError> for EngineError {
    fn from(e: StoreError) -> Self {
        fail(Code::RequestFailed, format!("store: {e}"))
    }
}

impl From<ChatError> for EngineError {
    fn from(e: ChatError) -> Self {
        fail(Code::RequestFailed, format!("chat message: {e}"))
    }
}

impl From<AgentFailure> for EngineError {
    fn from(e: AgentFailure) -> Self {
        let code = match &e {
            AgentFailure::Network(_) => Code::NoConnection,
            AgentFailure::BadLink(_) => Code::BadLink,
            AgentFailure::Other(_) => Code::RequestFailed,
        };
        fail(code, e.detail())
    }
}
