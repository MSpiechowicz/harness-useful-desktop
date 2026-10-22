use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Stable error category returned over IPC; the UI branches on this, never on the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IpcErrorCode {
    /// The request was malformed or failed validation.
    InvalidInput,
    /// A file, binary, session or other resource does not exist.
    NotFound,
    /// The operation is refused by a security rule (deny-list, path containment, ...).
    Forbidden,
    /// The harness or platform does not support the operation.
    Unsupported,
    /// A filesystem or I/O failure.
    Io,
    /// A child process failed to start or exited unexpectedly.
    Process,
    /// An operation exceeded its time limit.
    Timeout,
    /// A harness spoke an unexpected protocol.
    Protocol,
    /// The thing changed since the user last saw it (a script edited after it was reviewed): the
    /// UI reloads it and asks again.
    Conflict,
    /// What the user reviewed changed meanwhile (an agent saved a held note or script again): the
    /// UI shows the new version for another review.
    Changed,
    /// The harness does not trust the project folder yet (Claude Code's own trust answer): the UI
    /// offers to start again with `trustOnce`, which only the user can choose.
    Untrusted,
    /// A bug or an unexpected internal state.
    Internal,
}

/// Error payload of every failed IPC command.
///
/// `message` is human-readable and must never contain secret values or file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IpcError {
    /// Machine-readable category.
    pub code: IpcErrorCode,
    /// Short description safe to display.
    pub message: String,
}
