//! Crate-wide error type.
//!
//! Every variant maps 1:1 onto an [`IpcErrorCode`]. Messages describe *what* failed (an operation,
//! a path, a binary name) and must never include secret values or file contents.

use std::io;

use crate::model::{IpcError, IpcErrorCode};

/// Result alias used throughout the business layer.
pub type Result<T, E = BusinessError> = std::result::Result<T, E>;

/// Business-layer failure. See the module docs for the message rules.
#[derive(Debug, thiserror::Error)]
pub enum BusinessError {
    /// The request was malformed or failed validation.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A resource does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// Refused by a security rule.
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// Not supported by the harness or platform.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// A filesystem or I/O failure; `context` names the operation (and path, if any).
    #[error("{context}: {source}")]
    Io {
        /// What was being done when the error happened.
        context: String,
        /// The underlying OS error (its text never contains file contents).
        #[source]
        source: io::Error,
    },

    /// A child process could not be started or failed.
    #[error("process error: {0}")]
    Process(String),

    /// An operation exceeded its time limit.
    #[error("timed out: {0}")]
    Timeout(String),

    /// A harness sent something the protocol does not allow.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// The thing changed since the user last saw it; reload and ask again.
    #[error("conflict: {0}")]
    Conflict(String),

    /// A bug or unexpected state.
    #[error("internal error: {0}")]
    Internal(String),
}

impl BusinessError {
    /// Wraps an I/O error, promoting "not found" and "permission denied" to their own codes.
    pub fn from_io(context: impl Into<String>, source: io::Error) -> Self {
        let context = context.into();

        match source.kind() {
            io::ErrorKind::NotFound => Self::NotFound(context),
            io::ErrorKind::PermissionDenied => Self::Forbidden(format!("{context}: permission denied")),
            _ => Self::Io { context, source },
        }
    }

    /// The IPC category of this error.
    pub fn code(&self) -> IpcErrorCode {
        match self {
            Self::InvalidInput(_) => IpcErrorCode::InvalidInput,
            Self::NotFound(_) => IpcErrorCode::NotFound,
            Self::Forbidden(_) => IpcErrorCode::Forbidden,
            Self::Unsupported(_) => IpcErrorCode::Unsupported,
            Self::Io { .. } => IpcErrorCode::Io,
            Self::Process(_) => IpcErrorCode::Process,
            Self::Timeout(_) => IpcErrorCode::Timeout,
            Self::Protocol(_) => IpcErrorCode::Protocol,
            Self::Conflict(_) => IpcErrorCode::Conflict,
            Self::Internal(_) => IpcErrorCode::Internal,
        }
    }
}

impl From<BusinessError> for IpcError {
    fn from(error: BusinessError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}
