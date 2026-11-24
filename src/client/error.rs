//! What a client call fails with.

use crate::store::StoreError;
use crate::types::{ErrorCode, RequestKind};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The store did not open: a wrong key, a damaged file, a folder that cannot be written.
    #[error("the store did not open: {0}")]
    Store(#[from] StoreError),
    /// The client was closed, or its engine stopped.
    #[error("the client is closed")]
    Closed,
    /// The answer did not come within the builder's timeout.
    #[error("no answer in time")]
    Timeout,
    /// The engine refused the request.
    #[error("{request:?} failed with {code}: {detail}")]
    Failed {
        request: RequestKind,
        code: ErrorCode,
        /// The engine's own words, for a log.
        detail: String,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An answer of the wrong shape; the dispatcher's matching makes this unreachable.
pub(super) fn unexpected(request: RequestKind) -> Error {
    Error::Failed {
        request,
        code: ErrorCode::RequestFailed,
        detail: "an unexpected answer".into(),
    }
}
